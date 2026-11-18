//! System information (network, memory, temperature, storage, WiFi quality) and reboot.

use std::collections::HashMap;

use serde::Serialize;

use crate::cmd;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkQuality {
    pub interface: String,
    pub ssid: Option<String>,
    pub band: Option<&'static str>,
    /// MHz
    pub frequency: Option<i64>,
    /// Percentage, 0-100
    pub link_quality: Option<i64>,
    /// dBm
    pub signal_level: Option<f64>,
    /// dBm
    pub noise_level: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub hostname: String,
    pub ip_addresses: Vec<String>,
    /// MB
    pub total_ram: u64,
    /// MB
    pub free_ram: u64,
    pub temperature: Option<f64>,
    pub os: String,
    pub mac_address: Option<String>,
    pub storage_type: Option<String>,
    pub model: String,
    pub network_quality: Option<NetworkQuality>,
}

pub async fn get_info() -> SystemInfo {
    let (ip_addresses, mac_address) = network_info();
    let memory = memory_info().await;
    SystemInfo {
        hostname: hostname(),
        ip_addresses,
        total_ram: bytes_to_mb(memory.total),
        free_ram: bytes_to_mb(memory.available),
        temperature: temperature().await,
        os: os_info().await,
        mac_address,
        storage_type: storage_info().await,
        model: pi_model().await,
        network_quality: network_quality().await,
    }
}

pub fn hostname() -> String {
    nix::unistd::gethostname().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

/// IPv4 addresses of all non-loopback interfaces, and the first non-zero MAC among them.
fn network_info() -> (Vec<String>, Option<String>) {
    use nix::net::if_::InterfaceFlags;

    let Ok(addrs) = nix::ifaddrs::getifaddrs() else {
        return (Vec::new(), None);
    };

    let mut names: Vec<String> = Vec::new();
    let mut ipv4: HashMap<String, Vec<String>> = HashMap::new();
    let mut macs: HashMap<String, [u8; 6]> = HashMap::new();

    for ifaddr in addrs {
        let name = ifaddr.interface_name.clone();
        if !names.contains(&name) {
            names.push(name.clone());
        }
        let Some(address) = ifaddr.address else { continue };
        if let Some(link) = address.as_link_addr() {
            if let Some(mac) = link.addr() {
                macs.insert(name, mac);
            }
        } else if let Some(inet) = address.as_sockaddr_in() {
            if !ifaddr.flags.contains(InterfaceFlags::IFF_LOOPBACK) {
                ipv4.entry(name).or_default().push(std::net::SocketAddrV4::from(*inet).ip().to_string());
            }
        }
    }

    let mut ip_addresses = Vec::new();
    let mut mac_address = None;
    for name in &names {
        for ip in ipv4.get(name).into_iter().flatten() {
            ip_addresses.push(ip.clone());
            if mac_address.is_none() {
                mac_address = macs.get(name).filter(|mac| **mac != [0; 6]).map(format_mac);
            }
        }
    }
    (ip_addresses, mac_address)
}

fn format_mac(mac: &[u8; 6]) -> String {
    mac.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(":")
}

pub struct Memory {
    /// Bytes
    pub total: u64,
    /// Bytes
    pub available: u64,
}

/// Total and available memory from `/proc/meminfo` (0 if unavailable).
pub async fn memory_info() -> Memory {
    let content = tokio::fs::read_to_string("/proc/meminfo").await.unwrap_or_default();
    let field = |key: &str| -> u64 {
        content
            .lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
            .and_then(|rest| rest.split_whitespace().next()?.parse::<u64>().ok())
            .map_or(0, |kb| kb * 1024)
    };
    let available = match field("MemAvailable") {
        0 => field("MemFree"),
        n => n,
    };
    Memory { total: field("MemTotal"), available }
}

fn bytes_to_mb(bytes: u64) -> u64 {
    (bytes as f64 / (1024.0 * 1024.0)).round() as u64
}

async fn read_trimmed(path: &str) -> Option<String> {
    tokio::fs::read_to_string(path).await.ok().map(|s| s.trim().to_string())
}

/// SD card type/name, NVMe model or the model of the first sda/nvme disk.
async fn storage_info() -> Option<String> {
    if let (Some(kind), Some(name)) = (
        read_trimmed("/sys/class/block/mmcblk0/device/type").await,
        read_trimmed("/sys/class/block/mmcblk0/device/name").await,
    ) {
        return Some(format!("{kind} ({name})"));
    }
    if let Some(model) = read_trimmed("/sys/class/block/nvme0n1/device/model").await {
        return Some(format!("NVMe ({model})"));
    }
    let output = cmd::run_shell(r#"lsblk -d -o NAME,MODEL | grep -E "sda|nvme" | head -1"#).await.ok()?;
    let line = output.stdout.trim();
    if line.is_empty() {
        return None;
    }
    let model = line.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
    Some(if model.is_empty() { "Unknown".into() } else { model })
}

/// CPU temperature in °C.
async fn temperature() -> Option<f64> {
    let millidegrees = read_trimmed("/sys/class/thermal/thermal_zone0/temp").await?;
    millidegrees.parse::<i64>().ok().map(|m| m as f64 / 1000.0)
}

async fn pi_model() -> String {
    match tokio::fs::read("/proc/device-tree/model").await {
        Ok(raw) => String::from_utf8_lossy(&raw).replace('\0', "").trim().to_string(),
        Err(_) => "Unknown Device".into(),
    }
}

/// `PRETTY_NAME` from `/etc/os-release`, else kernel name and release.
async fn os_info() -> String {
    if let Ok(content) = tokio::fs::read_to_string("/etc/os-release").await {
        let pretty = content.lines().find_map(|line| line.strip_prefix("PRETTY_NAME=\"")?.strip_suffix('"'));
        if let Some(name) = pretty.filter(|n| !n.is_empty()) {
            return name.to_string();
        }
    }
    match nix::sys::utsname::uname() {
        Ok(uts) => format!("{} {}", uts.sysname().to_string_lossy(), uts.release().to_string_lossy()),
        Err(_) => "Unknown".into(),
    }
}

/// Value of `field` (e.g. `ssid`, `freq`) of the active WiFi connection, via nmcli.
async fn active_wifi_field(field: &str) -> Option<String> {
    let fields = format!("active,{field}");
    let output = cmd::run("nmcli", &["-t", "-f", &fields, "dev", "wifi"]).await.ok()?;
    let line = output.stdout.split('\n').find(|line| line.starts_with("yes:"))?.to_string();
    let value = line["yes:".len()..].trim();
    (!value.is_empty()).then(|| value.to_string())
}

pub fn band_from_frequency(freq: Option<i64>) -> Option<&'static str> {
    match freq? {
        2400..=2499 => Some("2.4GHz"),
        5000..=5899 => Some("5GHz"),
        5900..=7199 => Some("6GHz"),
        _ => None,
    }
}

/// WiFi link quality and signal from `/proc/net/wireless`. `None` without WiFi.
async fn network_quality() -> Option<NetworkQuality> {
    let wireless = tokio::fs::read_to_string("/proc/net/wireless").await.ok()?;
    let mut quality = parse_wireless(&wireless)?;
    quality.ssid = active_wifi_field("ssid").await;
    quality.frequency = active_wifi_field("freq").await.and_then(|f| parse_int_prefix(&f));
    quality.band = band_from_frequency(quality.frequency);
    Some(quality)
}

pub fn parse_wireless(content: &str) -> Option<NetworkQuality> {
    // Skip the two header rows
    let line = content.trim().split('\n').nth(2)?;
    if line.trim().is_empty() {
        return None;
    }
    let line = line.replacen(':', " ", 1);
    let parts: Vec<&str> = line.split_whitespace().collect();
    let field = |i: usize| parts.get(i).and_then(|s| parse_float_prefix(s));

    // Link quality is conventionally reported on a 0-70 scale
    let link_quality = field(2).map(|raw| ((raw / 70.0 * 100.0 + 0.5).floor() as i64).min(100));

    Some(NetworkQuality {
        interface: parts.first().copied().unwrap_or_default().to_string(),
        ssid: None,
        band: None,
        frequency: None,
        link_quality,
        signal_level: field(3),
        noise_level: field(4),
    })
}

/// Parses the longest numeric prefix, like JavaScript's `parseFloat` (e.g. `"-41."` → -41).
fn parse_float_prefix(s: &str) -> Option<f64> {
    let s = s.trim_start();
    let end = s
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit() || matches!(*c, '+' | '-' | '.' | 'e' | 'E'))
        .last()
        .map_or(0, |(i, c)| i + c.len_utf8());
    (1..=end).rev().find_map(|len| s[..len].parse::<f64>().ok()).filter(|v| v.is_finite())
}

/// Parses the leading integer, like JavaScript's `parseInt` (e.g. `"5180 MHz"` → 5180).
fn parse_int_prefix(s: &str) -> Option<i64> {
    let s = s.trim_start();
    let end = s
        .char_indices()
        .take_while(|(i, c)| c.is_ascii_digit() || (*i == 0 && matches!(*c, '+' | '-')))
        .last()
        .map_or(0, |(i, _)| i + 1);
    s[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_net_wireless() {
        let content = "Inter-| sta-|   Quality        |   Discarded packets               | Missed | WE\n face | tus | link level noise |  nwid  crypt   frag  retry   misc | beacon | 22\n wlan0: 0000   56.  -54.  -256        0      0      0      0      0        0\n";
        let q = parse_wireless(content).unwrap();
        assert_eq!(q.interface, "wlan0");
        assert_eq!(q.link_quality, Some(80));
        assert_eq!(q.signal_level, Some(-54.0));
        assert_eq!(q.noise_level, Some(-256.0));
    }

    #[test]
    fn no_wireless_interface() {
        let content = "Inter-| sta-|   Quality\n face | tus | link level noise\n";
        assert!(parse_wireless(content).is_none());
    }

    #[test]
    fn bands() {
        assert_eq!(band_from_frequency(Some(2412)), Some("2.4GHz"));
        assert_eq!(band_from_frequency(Some(5180)), Some("5GHz"));
        assert_eq!(band_from_frequency(Some(5955)), Some("6GHz"));
        assert_eq!(band_from_frequency(Some(900)), None);
        assert_eq!(band_from_frequency(None), None);
    }

    #[test]
    fn js_like_number_parsing() {
        assert_eq!(parse_float_prefix("-41."), Some(-41.0));
        assert_eq!(parse_float_prefix("70.5x"), Some(70.5));
        assert_eq!(parse_float_prefix("abc"), None);
        assert_eq!(parse_int_prefix("5180 MHz"), Some(5180));
        assert_eq!(parse_int_prefix(""), None);
    }
}

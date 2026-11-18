//! CamillaDSP config management for the main instance and the preamp instance.
//!
//! The active config of an instance is the symlink `client_config.yml` in its
//! config folder; the systemd service of the instance reads that file.

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use serde_yaml::{Mapping, Value};

use crate::cmd;
use crate::hats;

fn env_or(name: &str, default: impl FnOnce() -> PathBuf) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).unwrap_or_else(default)
}

/// Writable runtime location of the CamillaDSP configs.
fn camilla_home() -> PathBuf {
    env_or("CAMILLA_HOME", || "/var/lib/beatnik-hardware-api/camilladsp".into())
}

/// Configs shipped with this project. Looked up next to the executable
/// (`/opt/beatnik-hardware-api/camilladsp`), then in the working directory.
pub fn bundled_camilla_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BUNDLED_CAMILLA_DIR") {
        return dir.into();
    }
    let next_to_exe =
        std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("camilladsp")));
    match next_to_exe {
        Some(dir) if dir.is_dir() => dir,
        _ => PathBuf::from("camilladsp"),
    }
}

#[derive(Debug, Clone)]
pub struct CamillaInstance {
    config_dir: PathBuf,
    config_path: PathBuf,
    service_name: String,
}

impl CamillaInstance {
    pub fn new(config_dir: PathBuf, config_path: PathBuf, service_name: String) -> Self {
        Self { config_dir, config_path, service_name }
    }

    pub fn main_from_env() -> Self {
        let config_dir = env_or("CAMILLA_CONFIG_DIR", || camilla_home().join("configs"));
        let config_path = env_or("CAMILLA_CONFIG_PATH", || config_dir.join("client_config.yml"));
        let service = std::env::var("CAMILLA_SERVICE_NAME").unwrap_or_else(|_| "camilladsp".into());
        Self::new(config_dir, config_path, service)
    }

    /// Second instance used as preamp (e.g. vinyl line-in) in front of Snapcast.
    pub fn preamp_from_env() -> Self {
        let config_dir = env_or("CAMILLA_PREAMP_CONFIG_DIR", || camilla_home().join("preamp"));
        let config_path = env_or("CAMILLA_PREAMP_CONFIG_PATH", || config_dir.join("client_config.yml"));
        let service =
            std::env::var("CAMILLA_PREAMP_SERVICE_NAME").unwrap_or_else(|_| "camilladsp-preamp".into());
        Self::new(config_dir, config_path, service)
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    fn active_config_file_name(&self) -> String {
        file_name(&self.config_path)
    }

    pub async fn restart_service(&self) -> Result<(), CamillaError> {
        cmd::run("sudo", &["systemctl", "restart", &self.service_name]).await.map_err(|error| {
            tracing::error!("Failed to restart {} service: {error}", self.service_name);
            CamillaError::new("Failed to restart CamillaDSP service")
        })?;
        Ok(())
    }

    fn validate_config_file_name<'a>(&self, file_name: &'a str) -> Result<&'a str, CamillaError> {
        let trimmed = file_name.trim();
        if trimmed.is_empty() {
            return Err(CamillaError::new("Config file name is required"));
        }
        if Path::new(trimmed).file_name().and_then(|n| n.to_str()) != Some(trimmed) {
            return Err(CamillaError::new("Invalid config file name"));
        }
        if !is_yaml(trimmed) {
            return Err(CamillaError::new("Only .yml or .yaml files are allowed"));
        }
        let active = self.active_config_file_name();
        if trimmed == active {
            return Err(CamillaError::new(format!("{active} cannot be selected as source config")));
        }
        Ok(trimmed)
    }

    /// YAML files (and symlinks) in the config folder, without the active config link.
    pub async fn list_available_configs(&self) -> io::Result<Vec<String>> {
        let active = self.active_config_file_name();
        let mut entries = tokio::fs::read_dir(&self.config_dir).await?;
        let mut configs = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if !(file_type.is_file() || file_type.is_symlink()) {
                continue;
            }
            let Ok(name) = entry.file_name().into_string() else { continue };
            if is_yaml(&name) && name != active {
                configs.push(name);
            }
        }
        configs.sort_by(|a, b| collation_key(a).cmp(&collation_key(b)).then_with(|| b.cmp(a)));
        Ok(configs)
    }

    /// File name the active config link points to.
    pub async fn get_default_config(&self) -> io::Result<String> {
        let meta = tokio::fs::symlink_metadata(&self.config_path).await?;
        if meta.file_type().is_symlink() {
            let target = tokio::fs::read_link(&self.config_path).await?;
            return Ok(file_name(&target));
        }
        Ok(file_name(&self.config_path))
    }

    /// Points the active config link to `file_name` (atomically via rename) and,
    /// if `hat_id` is given, patches the HAT output device into it.
    pub async fn set_default_config(
        &self,
        file_name: &str,
        hat_id: Option<&str>,
    ) -> Result<(), CamillaError> {
        let safe_name = self.validate_config_file_name(file_name)?;
        let config_dir = std::path::absolute(&self.config_dir).map_err(CamillaError::from)?;
        let target = config_dir.join(safe_name);
        if target.parent() != Some(config_dir.as_path()) {
            return Err(CamillaError::new("Selected config path is outside of config directory"));
        }

        let meta = tokio::fs::metadata(&target).await.map_err(|error| {
            CamillaError::new(format!("Selected config {safe_name} cannot be read: {error}"))
        })?;
        if !meta.is_file() {
            return Err(CamillaError::new("Selected config is not a file"));
        }

        let mut tmp_link = self.config_path.clone().into_os_string();
        tmp_link.push(".tmp");
        let tmp_link = PathBuf::from(tmp_link);
        let _ = tokio::fs::remove_file(&tmp_link).await;
        tokio::fs::symlink(&target, &tmp_link).await?;
        tokio::fs::rename(&tmp_link, &self.config_path).await?;

        if let Some(hat_id) = hat_id {
            self.update_config(hat_id).await?;
        }
        Ok(())
    }

    /// Sets the HAT's output device in the active config, keeping filters/mixers.
    pub async fn update_config(&self, hat_id: &str) -> Result<(), CamillaError> {
        if hats::find(hat_id).is_none() {
            return Err(CamillaError::new("Invalid HAT ID"));
        }
        let result: Result<(), Box<dyn std::error::Error + Send + Sync>> = async {
            let content = tokio::fs::read_to_string(&self.config_path).await?;
            let patched = patch_playback_device(&content, hat_id)?;
            // Writes through the symlink into the selected config file
            tokio::fs::write(&self.config_path, patched).await?;
            Ok(())
        }
        .await;

        match result {
            Ok(()) => {
                let device = hats::find(hat_id).map(|hat| hat.camilla.device).unwrap_or_default();
                tracing::info!("CamillaDSP updated for HAT: {hat_id} -> Device: {device}");
                Ok(())
            }
            Err(error) => {
                tracing::error!("Failed to update CamillaDSP config: {error}");
                Err(CamillaError::new("Failed to update CamillaDSP config"))
            }
        }
    }
}

/// Sets `devices.playback.device/format(/channels)` for the HAT and returns the new YAML.
pub fn patch_playback_device(yaml: &str, hat_id: &str) -> Result<String, serde_yaml::Error> {
    let hat = hats::find(hat_id).expect("hat_id is validated by the caller");
    let mut config: Value = serde_yaml::from_str(yaml)?;
    if !config.is_mapping() {
        config = Value::Mapping(Mapping::new());
    }
    let root = config.as_mapping_mut().expect("is a mapping");

    // Remove incorrect top-level playback (cleanup from an old bug)
    root.remove("playback");

    let devices = ensure_mapping(root, "devices");
    let playback = ensure_mapping(devices, "playback");
    playback.insert("device".into(), hat.camilla.device.into());
    playback.insert("format".into(), hat.camilla.format.into());
    if let Some(channels) = hat.camilla.channels {
        playback.insert("channels".into(), channels.into());
    }

    serde_yaml::to_string(&config)
}

/// Returns the mapping at `key`, replacing a missing or non-mapping value with `{}`.
fn ensure_mapping<'a>(map: &'a mut Mapping, key: &str) -> &'a mut Mapping {
    let value = map.entry(key.into()).or_insert(Value::Mapping(Mapping::new()));
    if !value.is_mapping() {
        *value = Value::Mapping(Mapping::new());
    }
    value.as_mapping_mut().expect("is a mapping")
}

fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Sort key approximating the ICU root collation used by JavaScript's
/// `localeCompare`: punctuation (in ICU order) < digits < letters, case-insensitive.
/// Ties are broken with lowercase first by the caller.
fn collation_key(name: &str) -> Vec<(u8, char)> {
    const PUNCTUATION: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
    name.chars()
        .map(|c| match PUNCTUATION.find(c) {
            Some(index) => (1, char::from(index as u8)),
            None if c.is_whitespace() => (0, ' '),
            None if c.is_ascii_digit() => (2, c),
            None => (3, c.to_lowercase().next().unwrap_or(c)),
        })
        .collect()
}

fn is_yaml(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yml") || ext.eq_ignore_ascii_case("yaml"))
}

/// Seeds the runtime config folders with the bundled configs. Existing files
/// are never overwritten, so user changes survive updates.
pub async fn seed_configs() -> io::Result<()> {
    let bundled = bundled_camilla_dir();
    let main = CamillaInstance::main_from_env();
    let preamp = CamillaInstance::preamp_from_env();
    let targets = [
        ("configs", main.config_dir().to_path_buf()),
        ("preamp", preamp.config_dir().to_path_buf()),
        ("coeffs", camilla_home().join("coeffs")),
    ];
    for (name, target) in targets {
        let source = bundled.join(name);
        tokio::task::spawn_blocking(move || copy_missing(&source, &target))
            .await
            .map_err(io::Error::other)??;
    }
    Ok(())
}

fn copy_missing(source_dir: &Path, target_dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(target_dir)?;
    for entry in std::fs::read_dir(source_dir)? {
        let entry = entry?;
        let source = entry.path();
        let target = target_dir.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_missing(&source, &target)?;
            continue;
        }
        // create_new keeps user-modified configs and symlinks (even dangling ones) untouched
        let mut output = match std::fs::OpenOptions::new().write(true).create_new(true).open(&target) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        let mut input = std::fs::File::open(&source)?;
        io::copy(&mut input, &mut output)?;
        output.set_permissions(input.metadata()?.permissions())?;
    }
    Ok(())
}

/// Error with a message that is safe to return to API clients.
#[derive(Debug)]
pub struct CamillaError(pub String);

impl CamillaError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for CamillaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<io::Error> for CamillaError {
    fn from(error: io::Error) -> Self {
        Self(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(dir: &Path) -> CamillaInstance {
        CamillaInstance::new(dir.to_path_buf(), dir.join("client_config.yml"), "camilladsp-test".into())
    }

    #[test]
    fn patches_playback_device_and_keeps_the_rest() {
        let yaml = "devices:\n  samplerate: 48000\n  playback:\n    type: Alsa\n    device: hw:0\nplayback: {}\nfilters:\n  f: {type: Gain}\n";
        let out: Value =
            serde_yaml::from_str(&patch_playback_device(yaml, "hifiberry-digi").unwrap()).unwrap();
        assert_eq!(out["devices"]["playback"]["device"], "plughw:CARD=sndrpihifiberry,DEV=0");
        assert_eq!(out["devices"]["playback"]["format"], "S24LE3");
        assert_eq!(out["devices"]["playback"]["type"], "Alsa");
        assert_eq!(out["devices"]["samplerate"], 48000);
        assert_eq!(out["filters"]["f"]["type"], "Gain");
        assert!(out.get("playback").is_none());
    }

    #[test]
    fn sorts_like_locale_compare() {
        // Order produced by the Node.js version (`localeCompare`)
        let expected = [
            "stereo-mixers.yaml",
            "stereo-pioneer_cs-e500-bass-boost.yaml",
            "stereo-pioneer_cs-e500.yaml",
            "stereo-pioneer-cs-77.yaml",
            "Stereo-sirus.yaml",
        ];
        let mut names = expected;
        names.reverse();
        names.sort_by(|a, b| collation_key(a).cmp(&collation_key(b)).then_with(|| b.cmp(a)));
        assert_eq!(names, expected);
    }

    #[test]
    fn creates_missing_devices_section() {
        let out: Value = serde_yaml::from_str(&patch_playback_device("{}", "none").unwrap()).unwrap();
        assert_eq!(out["devices"]["playback"]["device"], "plughw:Headphones");
    }

    #[test]
    fn patches_all_bundled_configs() {
        for dir in ["camilladsp/configs", "camilladsp/preamp"] {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if is_yaml(&path.to_string_lossy()) {
                    let yaml = std::fs::read_to_string(&path).unwrap();
                    patch_playback_device(&yaml, "hifiberry-amp")
                        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                }
            }
        }
    }

    #[tokio::test]
    async fn lists_and_switches_configs() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["b.yaml", "A.yml", "notes.txt", "client_config.yml"] {
            std::fs::write(dir.path().join(name), "devices: {}\n").unwrap();
        }
        let camilla = instance(dir.path());

        assert_eq!(camilla.list_available_configs().await.unwrap(), vec!["A.yml", "b.yaml"]);
        assert_eq!(camilla.get_default_config().await.unwrap(), "client_config.yml");

        camilla.set_default_config(" b.yaml ", Some("hifiberry-amp")).await.unwrap();
        assert_eq!(camilla.get_default_config().await.unwrap(), "b.yaml");
        let patched = std::fs::read_to_string(dir.path().join("b.yaml")).unwrap();
        assert!(patched.contains("plughw:CARD=sndrpihifiberry,DEV=0"));
        // The link itself is not listed
        assert_eq!(camilla.list_available_configs().await.unwrap(), vec!["A.yml", "b.yaml"]);
    }

    #[tokio::test]
    async fn rejects_invalid_file_names() {
        let dir = tempfile::tempdir().unwrap();
        let camilla = instance(dir.path());
        for (name, message) in [
            ("", "Config file name is required"),
            ("../x.yml", "Invalid config file name"),
            ("x.txt", "Only .yml or .yaml files are allowed"),
            ("client_config.yml", "client_config.yml cannot be selected as source config"),
        ] {
            let error = camilla.set_default_config(name, None).await.unwrap_err();
            assert_eq!(error.0, message);
        }
    }

    #[test]
    fn seeding_does_not_overwrite() {
        let source = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        std::fs::create_dir(source.path().join("sub")).unwrap();
        std::fs::write(source.path().join("a.yml"), "new").unwrap();
        std::fs::write(source.path().join("sub/b.yml"), "new").unwrap();
        std::fs::write(target.path().join("a.yml"), "user").unwrap();

        copy_missing(source.path(), target.path()).unwrap();
        assert_eq!(std::fs::read_to_string(target.path().join("a.yml")).unwrap(), "user");
        assert_eq!(std::fs::read_to_string(target.path().join("sub/b.yml")).unwrap(), "new");
    }
}

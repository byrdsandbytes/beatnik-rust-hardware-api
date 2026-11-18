//! Background updates of beatnik-bleno, the controller container and this API.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::cmd::{self, CmdError, RunOptions};

const UPDATE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const MAX_OUTPUT_CHARS: usize = 4000;
const DOWNLOAD_DIR: &str = "/tmp/beatnik-update";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateTarget {
    Bleno,
    Controller,
    Api,
}

/// All targets; also the order of an `all` update. The API goes last because
/// its update restarts this process.
pub const UPDATE_TARGETS: [UpdateTarget; 3] =
    [UpdateTarget::Bleno, UpdateTarget::Controller, UpdateTarget::Api];

impl UpdateTarget {
    pub fn parse(name: &str) -> Option<Self> {
        UPDATE_TARGETS.into_iter().find(|t| t.as_str() == name)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bleno => "bleno",
            Self::Controller => "controller",
            Self::Api => "api",
        }
    }

    /// Targets installed as Debian packages and updated from the latest GitHub release.
    fn deb(self) -> Option<DebTarget> {
        match self {
            Self::Bleno => Some(DebTarget {
                package_name: "beatnik-bleno",
                repo: "byrdsandbytes/beatnik-bleno",
                detached: false,
            }),
            // The API restarts itself during the update, so apt must run outside this service
            Self::Api => Some(DebTarget {
                package_name: "beatnik-hardware-api",
                repo: "byrdsandbytes/beatnik-hardware-api",
                detached: true,
            }),
            Self::Controller => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DebTarget {
    package_name: &'static str,
    repo: &'static str,
    detached: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateState {
    Idle,
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub target: UpdateTarget,
    pub state: UpdateState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

impl UpdateStatus {
    fn idle(target: UpdateTarget) -> Self {
        Self {
            target,
            state: UpdateState::Idle,
            started_at: None,
            finished_at: None,
            message: None,
            output: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionInfo {
    pub target: UpdateTarget,
    pub installed: bool,
    pub version: Option<String>,
    /// Controller only: Docker image reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Controller only: ID of the image of the running container
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_id: Option<String>,
}

/// Error of [`UpdateService::start`], with the HTTP status to answer with.
#[derive(Debug)]
pub struct UpdateError {
    pub message: String,
    pub status: StatusCode,
}

/// Result of a successful update step.
struct Done {
    message: String,
    output: Option<String>,
}

/// A failed update step, with the output of the failed command if any.
#[derive(Debug)]
struct Failure {
    message: String,
    stdout: String,
    stderr: String,
}

impl From<CmdError> for Failure {
    fn from(e: CmdError) -> Self {
        Self { message: e.message, stdout: e.stdout, stderr: e.stderr }
    }
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Self { message, stdout: String::new(), stderr: String::new() }
    }
}

impl From<std::io::Error> for Failure {
    fn from(e: std::io::Error) -> Self {
        e.to_string().into()
    }
}

impl From<reqwest::Error> for Failure {
    fn from(e: reqwest::Error) -> Self {
        e.to_string().into()
    }
}

impl Failure {
    /// Last stderr line, else the first line of the error message.
    fn summary(&self) -> String {
        match self.stderr.trim().lines().last() {
            Some(line) if !line.is_empty() => line.to_string(),
            _ => self.message.lines().next().unwrap_or("Update failed").to_string(),
        }
    }
}

struct State {
    statuses: Vec<UpdateStatus>,
    busy: bool,
}

#[derive(Clone)]
pub struct UpdateService {
    state: Arc<Mutex<State>>,
    http: reqwest::Client,
    beatnik_home: PathBuf,
    controller_container: String,
    controller_image: Option<String>,
}

impl UpdateService {
    pub fn from_env() -> Self {
        // The API runs as root, but the controller lives in the beatnik user's home directory
        let user = std::env::var("BEATNIK_USER").unwrap_or_else(|_| "beatnik".into());
        let home = std::env::var("BEATNIK_HOME").unwrap_or_else(|_| format!("/home/{user}"));
        let http = reqwest::Client::builder()
            .user_agent("beatnik-hardware-api")
            .build()
            .expect("HTTP client can be built");
        Self {
            state: Arc::new(Mutex::new(State {
                statuses: UPDATE_TARGETS.map(UpdateStatus::idle).to_vec(),
                busy: false,
            })),
            http,
            beatnik_home: home.into(),
            controller_container: std::env::var("CONTROLLER_CONTAINER")
                .unwrap_or_else(|_| "beatnik-controller".into()),
            controller_image: std::env::var("CONTROLLER_IMAGE").ok().filter(|s| !s.is_empty()),
        }
    }

    pub fn is_busy(&self) -> bool {
        self.state.lock().unwrap().busy
    }

    pub fn get_status(&self) -> Vec<UpdateStatus> {
        self.state.lock().unwrap().statuses.clone()
    }

    fn set_status(&self, status: UpdateStatus) {
        let mut state = self.state.lock().unwrap();
        if let Some(slot) = state.statuses.iter_mut().find(|s| s.target == status.target) {
            *slot = status;
        }
    }

    /// Validates the request (`None` = all targets) and starts the update in
    /// the background. Returns the targets that will be updated.
    pub async fn start(&self, request: Option<UpdateTarget>) -> Result<Vec<UpdateTarget>, UpdateError> {
        {
            let mut state = self.state.lock().unwrap();
            if state.busy {
                return Err(UpdateError {
                    message: "An update is already running".into(),
                    status: StatusCode::CONFLICT,
                });
            }
            state.busy = true;
        }

        let targets = request.map_or_else(|| UPDATE_TARGETS.to_vec(), |t| vec![t]);
        for &target in &targets {
            if let Err(error) = self.assert_available(target).await {
                self.state.lock().unwrap().busy = false;
                return Err(error);
            }
        }

        for &target in &targets {
            self.set_status(UpdateStatus::idle(target));
        }
        let service = self.clone();
        let sequence = targets.clone();
        tokio::spawn(async move {
            for target in sequence {
                if !service.run_target(target).await {
                    break;
                }
            }
            service.state.lock().unwrap().busy = false;
        });
        Ok(targets)
    }

    async fn assert_available(&self, target: UpdateTarget) -> Result<(), UpdateError> {
        // The controller only needs the container, not a package
        let Some(deb) = target.deb() else { return Ok(()) };
        if installed_version(deb.package_name).await.is_none() {
            return Err(UpdateError {
                message: format!(
                    "Cannot update {}: package {} is not installed",
                    target.as_str(),
                    deb.package_name
                ),
                status: StatusCode::NOT_FOUND,
            });
        }
        Ok(())
    }

    async fn run_target(&self, target: UpdateTarget) -> bool {
        let started_at = Some(now_iso());
        self.set_status(UpdateStatus {
            state: UpdateState::Running,
            started_at: started_at.clone(),
            ..UpdateStatus::idle(target)
        });

        let result = match target {
            UpdateTarget::Bleno | UpdateTarget::Api => self.update_deb(target).await,
            UpdateTarget::Controller => self.update_controller().await,
        };

        let mut status =
            UpdateStatus { started_at, finished_at: Some(now_iso()), ..UpdateStatus::idle(target) };
        let ok = match result {
            Ok(done) => {
                status.state = UpdateState::Success;
                status.message = Some(done.message);
                status.output = done.output;
                true
            }
            Err(failure) => {
                tracing::error!("Update of {} failed: {}", target.as_str(), failure.message);
                status.state = UpdateState::Failed;
                status.message = Some(failure.summary());
                status.output = Some(tail(&format!("{}\n{}", failure.stdout, failure.stderr)));
                false
            }
        };
        self.set_status(status);
        ok
    }

    /// Runs docker with root privileges (via sudo when not already root).
    async fn docker(&self, args: &[&str], cwd: Option<&Path>) -> Result<cmd::Output, CmdError> {
        let options = RunOptions { cwd, timeout: Some(UPDATE_TIMEOUT) };
        if cmd::is_root() {
            cmd::run_with("docker", args, options).await
        } else {
            let sudo_args: Vec<&str> = ["-n", "docker"].iter().chain(args).copied().collect();
            cmd::run_with("sudo", &sudo_args, options).await
        }
    }

    async fn docker_inspect(&self, args: &[&str]) -> String {
        self.docker(args, None).await.map(|o| o.stdout.trim().to_string()).unwrap_or_default()
    }

    async fn update_controller(&self) -> Result<Done, Failure> {
        let dir = self.beatnik_home.join("beatnik-controller");
        if tokio::fs::try_exists(dir.join("docker-compose.yml")).await.unwrap_or(false) {
            self.update_controller_with_compose(&dir).await
        } else {
            self.update_controller_with_pull().await
        }
    }

    /// Pulls the image and recreates the container if the image changed.
    async fn update_controller_with_compose(&self, dir: &Path) -> Result<Done, Failure> {
        let inspect = ["inspect", "--format", "{{.Image}}", self.controller_container.as_str()];
        let before = self.docker_inspect(&inspect).await;
        let pull = self.docker(&["compose", "pull"], Some(dir)).await?;
        let up = self.docker(&["compose", "up", "-d"], Some(dir)).await?;
        let after = self.docker_inspect(&inspect).await;

        Ok(Done {
            message: if before != after {
                "Controller updated and container recreated".into()
            } else {
                "Controller is already up to date".into()
            },
            output: Some(tail(&format!("{}\n{}\n{}\n{}", pull.stdout, pull.stderr, up.stdout, up.stderr))),
        })
    }

    /// Fallback without a docker-compose.yml: pull only, the container is not recreated.
    async fn update_controller_with_pull(&self) -> Result<Done, Failure> {
        let image = self.controller_image().await?;
        let inspect = ["image", "inspect", "--format", "{{.Id}}", image.as_str()];
        let before = self.docker_inspect(&inspect).await;
        let pull = self.docker(&["pull", &image], None).await?;
        let after = self.docker_inspect(&inspect).await;

        Ok(Done {
            message: if before != after {
                format!("Pulled new controller image {image}. Recreate the container to use it.")
            } else {
                format!("Controller image {image} is already up to date")
            },
            output: Some(tail(&format!("{}\n{}", pull.stdout, pull.stderr))),
        })
    }

    async fn controller_image(&self) -> Result<String, Failure> {
        if let Some(image) = &self.controller_image {
            return Ok(image.clone());
        }
        let image = self
            .docker_inspect(&["inspect", "--format", "{{.Config.Image}}", &self.controller_container])
            .await;
        if !image.is_empty() {
            return Ok(image);
        }
        Err(format!(
            "Could not determine controller image from container \"{}\". Set CONTROLLER_IMAGE.",
            self.controller_container
        )
        .into())
    }

    /// Installed versions of the API, bleno (Debian packages) and the controller (Docker image).
    pub async fn get_versions(&self) -> Vec<VersionInfo> {
        let (bleno, controller, api) = tokio::join!(
            self.deb_version(UpdateTarget::Bleno),
            self.controller_version(),
            self.deb_version(UpdateTarget::Api),
        );
        vec![bleno, controller, api]
    }

    async fn deb_version(&self, target: UpdateTarget) -> VersionInfo {
        let deb = target.deb().expect("target is a Debian package");
        let mut version = installed_version(deb.package_name).await;
        if version.is_none() && target == UpdateTarget::Api {
            // Not installed as a package (e.g. built from source)
            version = Some(env!("CARGO_PKG_VERSION").to_string());
        }
        VersionInfo { target, installed: version.is_some(), version, image: None, image_id: None }
    }

    async fn controller_version(&self) -> VersionInfo {
        let format =
            r#"{{.Config.Image}}|{{.Image}}|{{index .Config.Labels "org.opencontainers.image.version"}}"#;
        match self.docker(&["inspect", "--format", format, &self.controller_container], None).await {
            Ok(output) => {
                let (image, image_id, label) = parse_controller_inspect(&output.stdout);
                let tag = match image.rsplit_once(':') {
                    Some((_, tag)) => tag.to_string(),
                    None => "latest".into(),
                };
                VersionInfo {
                    target: UpdateTarget::Controller,
                    installed: true,
                    version: Some(label.unwrap_or(tag)),
                    image: Some(image),
                    image_id: Some(image_id),
                }
            }
            Err(_) => VersionInfo {
                target: UpdateTarget::Controller,
                installed: false,
                version: None,
                image: None,
                image_id: None,
            },
        }
    }

    /// Finds the latest release and the .deb asset matching this device's architecture.
    async fn latest_deb(&self, deb: DebTarget) -> Result<LatestDeb, Failure> {
        let url = format!("https://api.github.com/repos/{}/releases/latest", deb.repo);
        let response = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(30))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(format!("GitHub release lookup failed (HTTP {})", response.status().as_u16()).into());
        }
        let release: Release = response.json().await?;
        let version = release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name).to_string();
        let arch = cmd::run("dpkg", &["--print-architecture"]).await?.stdout.trim().to_string();

        let prefix = format!("{}_", deb.package_name);
        let arch_suffix = format!("_{arch}.deb");
        let asset = release
            .assets
            .into_iter()
            .find(|a| {
                a.name.starts_with(&prefix)
                    && (a.name.ends_with(&arch_suffix) || a.name.ends_with("_all.deb"))
            })
            .ok_or_else(|| {
                format!("No {} .deb for {arch} found in release {}", deb.package_name, release.tag_name)
            })?;
        Ok(LatestDeb { version, name: asset.name, url: asset.browser_download_url })
    }

    async fn download(&self, url: &str, name: &str) -> Result<PathBuf, Failure> {
        use std::os::unix::fs::PermissionsExt;

        tokio::fs::create_dir_all(DOWNLOAD_DIR).await?;
        tokio::fs::set_permissions(DOWNLOAD_DIR, std::fs::Permissions::from_mode(0o755)).await?;
        let file_name = Path::new(name).file_name().ok_or_else(|| format!("Invalid asset name {name}"))?;
        let path = Path::new(DOWNLOAD_DIR).join(file_name);

        let response = self.http.get(url).timeout(UPDATE_TIMEOUT).send().await?;
        if !response.status().is_success() {
            return Err(format!("Download failed (HTTP {})", response.status().as_u16()).into());
        }
        let bytes = response.bytes().await?;
        tokio::fs::write(&path, &bytes).await?;
        tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).await?;
        Ok(path)
    }

    /// Downloads the latest release .deb and installs it with apt, which upgrades
    /// the package in place and restarts its service.
    async fn update_deb(&self, target: UpdateTarget) -> Result<Done, Failure> {
        let deb = target.deb().expect("target is a Debian package");
        let installed = installed_version(deb.package_name).await;
        let latest = self.latest_deb(deb).await?;
        let installed_label = installed.clone().unwrap_or_else(|| "none".into());

        if installed.as_deref() == Some(latest.version.as_str()) {
            return Ok(Done {
                message: format!("{} is already up to date ({installed_label})", deb.package_name),
                output: None,
            });
        }

        let path = self.download(&latest.url, &latest.name).await?;
        let path = path.to_string_lossy().into_owned();
        // Keep locally edited service files, no prompt
        let apt_args = ["install", "-y", "-o", "Dpkg::Options::=--force-confold", path.as_str()];
        let sudo: &[&str] = if cmd::is_root() { &[] } else { &["sudo", "-n"] };

        if deb.detached {
            // Own systemd unit so apt survives the restart of this service
            let millis = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
            let unit = format!("beatnik-api-update-{millis}");
            let unit_arg = format!("--unit={unit}");
            let command: Vec<&str> = sudo
                .iter()
                .copied()
                .chain([
                    "systemd-run",
                    unit_arg.as_str(),
                    "--collect",
                    "--setenv=DEBIAN_FRONTEND=noninteractive",
                    "apt-get",
                ])
                .chain(apt_args)
                .collect();
            let options = RunOptions { cwd: None, timeout: Some(Duration::from_secs(30)) };
            cmd::run_with(command[0], &command[1..], options).await?;
            return Ok(Done {
                message: format!(
                    "Updating {} {installed_label} -> {}. The service will restart shortly.",
                    deb.package_name, latest.version
                ),
                output: Some(format!("Running as systemd unit {unit}. Follow with: journalctl -u {unit}")),
            });
        }

        let command: Vec<&str> = sudo
            .iter()
            .copied()
            .chain(["env", "DEBIAN_FRONTEND=noninteractive", "apt-get"])
            .chain(apt_args)
            .collect();
        let options = RunOptions { cwd: None, timeout: Some(UPDATE_TIMEOUT) };
        let output = cmd::run_with(command[0], &command[1..], options).await?;
        Ok(Done {
            message: format!("{} updated {installed_label} -> {}", deb.package_name, latest.version),
            output: Some(tail(&format!("{}\n{}", output.stdout, output.stderr))),
        })
    }
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

struct LatestDeb {
    version: String,
    name: String,
    url: String,
}

/// Installed version of a Debian package, `None` if not installed.
async fn installed_version(package: &str) -> Option<String> {
    let output = cmd::run("dpkg-query", &["-W", "-f=${Version}", package]).await.ok()?;
    let version = output.stdout.trim();
    (!version.is_empty()).then(|| version.to_string())
}

/// Splits `image|imageId|label`; a missing label (`<no value>`) becomes `None`.
fn parse_controller_inspect(stdout: &str) -> (String, String, Option<String>) {
    let mut parts = stdout.trim().split('|');
    let image = parts.next().unwrap_or_default().to_string();
    let image_id = parts.next().unwrap_or_default().to_string();
    let label = parts.next().filter(|l| !l.is_empty() && *l != "<no value>").map(str::to_string);
    (image, image_id, label)
}

/// The last [`MAX_OUTPUT_CHARS`] characters of the trimmed text.
fn tail(text: &str) -> String {
    let text = text.trim();
    let count = text.chars().count();
    text.chars().skip(count.saturating_sub(MAX_OUTPUT_CHARS)).collect()
}

/// Current time like JavaScript's `Date.toISOString()`, e.g. `2024-01-01T12:00:00.000Z`.
fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_targets() {
        assert_eq!(UpdateTarget::parse("api"), Some(UpdateTarget::Api));
        assert_eq!(UpdateTarget::parse("all"), None);
    }

    #[test]
    fn tail_keeps_the_end() {
        let long = "x".repeat(5000) + "end";
        let t = tail(&long);
        assert_eq!(t.chars().count(), MAX_OUTPUT_CHARS);
        assert!(t.ends_with("end"));
        assert_eq!(tail("  short \n"), "short");
    }

    #[test]
    fn controller_inspect() {
        assert_eq!(
            parse_controller_inspect("ghcr.io/x/y:1.2|sha256:abc|<no value>\n"),
            ("ghcr.io/x/y:1.2".into(), "sha256:abc".into(), None)
        );
        assert_eq!(parse_controller_inspect("img|id|2.0").2, Some("2.0".into()));
    }

    #[test]
    fn failure_summary() {
        let f = Failure {
            message: "Command failed: x\nboom".into(),
            stdout: String::new(),
            stderr: "a\nE: last\n".into(),
        };
        assert_eq!(f.summary(), "E: last");
        let f: Failure = String::from("first\nsecond").into();
        assert_eq!(f.summary(), "first");
    }

    #[test]
    fn status_json_omits_unset_fields() {
        let json = serde_json::to_value(UpdateStatus::idle(UpdateTarget::Bleno)).unwrap();
        assert_eq!(json, serde_json::json!({ "target": "bleno", "state": "idle" }));
    }

    #[test]
    fn iso_timestamp_format() {
        let ts = now_iso();
        assert_eq!(ts.len(), 24);
        assert!(ts.ends_with('Z'));
    }
}

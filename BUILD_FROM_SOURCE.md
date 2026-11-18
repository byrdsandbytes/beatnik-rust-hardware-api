# Beatnik Hardware API: Build from Source

This guide is for developers who want to build, run, and modify the Beatnik Hardware API from source. For a regular production install, use the [Installation & Updating Guide](INSTALL.md) instead.

---

## Prerequisites

- **Rust** (stable, via [rustup](https://rustup.rs)): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- **Git**
- Optional: a Raspberry Pi running Raspberry Pi OS (Bookworm or newer) for testing against real hardware. The API can also run on Linux/macOS with test files (see [Run Locally Without a Pi](#run-locally-without-a-pi)).

## 1. Get the Source

```bash
git clone https://github.com/byrdsandbytes/beatnik-hardware-api.git
cd beatnik-hardware-api
```

## 2. Project Layout

```text
src/
  main.rs            Entry point (axum, port 3000), CORS, mDNS start/stop, graceful shutdown
  routes/            HTTP routes: audio (/api/hardware), system, snapcast, update
  boot_config.rs     Reads/writes /boot/firmware/config.txt
  camilla.rs         CamillaDSP configs (seeding, active config symlink, output device patching)
  detection.rs       HAT detection via the EEPROM in the device tree
  hats.rs            Supported audio HATs
  mdns.rs            Avahi service file
  snapcast.rs        Snapserver/Snapclient control
  system.rs          System info (network, memory, WiFi, storage)
  update.rs          Updates of bleno, the controller and this API
  cmd.rs             Running external commands
camilladsp/          Bundled CamillaDSP configs (shipped next to the binary)
examples/            Angular client examples for the API
setup.sh             Production installer/updater (downloads the release artifact)
beatnik-hardware.service   systemd unit template
```

## 3. Run in Development Mode

```bash
cargo run
```

This builds a debug binary and starts the server on `http://0.0.0.0:3000` (override with `PORT`). Log verbosity is controlled by `RUST_LOG`, e.g. `RUST_LOG=debug cargo run`. For auto-restart on file changes use [cargo-watch](https://crates.io/crates/cargo-watch): `cargo watch -x run`.

Check that it is running:
```bash
curl http://localhost:3000/api/system/info
```

### Run Locally Without a Pi

By default the API reads and writes system files such as `/boot/firmware/config.txt` and `/var/lib/beatnik-hardware-api/camilladsp`. To develop on any machine, point it at local files:

```bash
touch test-config.txt
mkdir -p test-avahi

CONFIG_PATH=./test-config.txt \
CAMILLA_HOME=./test-camilla \
AVAHI_SERVICE_DIR=./test-avahi \
cargo run
```

On start, the bundled configs are copied into `./test-camilla`. Anything that needs `systemctl`, `docker`, or Avahi (Snapcast, update routes, restarting CamillaDSP) will fail or log errors on a machine without them; this is expected. Never run the update routes on a development machine, as they install `.deb` packages with apt and run `docker compose` against `$BEATNIK_HOME`.

### Environment Variables

| Variable | Default | Purpose |
|---|---|---|
| `PORT` | `3000` | HTTP port (also advertised via mDNS) |
| `RUST_LOG` | `info` | Log filter, e.g. `debug` or `info,tower_http=warn` |
| `CONFIG_PATH` | `/boot/firmware/config.txt` | Raspberry Pi boot config |
| `CAMILLA_HOME` | `/var/lib/beatnik-hardware-api/camilladsp` | Writable CamillaDSP directory (`configs`, `preamp`, `coeffs`). On start, missing files are copied from the bundled `camilladsp/` folder; existing files are never overwritten |
| `BUNDLED_CAMILLA_DIR` | `camilladsp/` next to the binary, else `./camilladsp` | Bundled CamillaDSP configs |
| `CAMILLA_CONFIG_DIR` | `<CAMILLA_HOME>/configs` | CamillaDSP config directory |
| `CAMILLA_CONFIG_PATH` | `<CAMILLA_CONFIG_DIR>/client_config.yml` | Active CamillaDSP config (symlink) |
| `CAMILLA_SERVICE_NAME` | `camilladsp` | systemd service restarted after config changes |
| `CAMILLA_PREAMP_CONFIG_DIR` | `<CAMILLA_HOME>/preamp` | Preamp CamillaDSP config directory |
| `CAMILLA_PREAMP_CONFIG_PATH` | `<CAMILLA_PREAMP_CONFIG_DIR>/client_config.yml` | Active preamp config (symlink) |
| `CAMILLA_PREAMP_SERVICE_NAME` | `camilladsp-preamp` | systemd service restarted after preamp config changes |
| `AVAHI_SERVICE_DIR` | `/etc/avahi/services` | Where the mDNS service file is written |
| `BEATNIK_USER` | `beatnik` | User that owns the Beatnik repositories (update routes) |
| `BEATNIK_HOME` | `/home/<BEATNIK_USER>` | Directory containing the Beatnik repositories (update routes) |
| `CONTROLLER_CONTAINER` | `beatnik-controller` | Docker container name (update routes) |
| `CONTROLLER_IMAGE` | read from container | Docker image to pull (update routes) |

## 4. Test and Build

```bash
cargo test                 # unit tests
cargo clippy --all-targets # lints
cargo fmt                  # formatting
cargo build --release      # optimized binary in target/release/beatnik-hardware-api
```

### Cross-compiling for the Pi

Building on the Pi works but is slow. To build on another machine, use [cross](https://github.com/cross-rs/cross) (needs Docker):

```bash
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu    # 64-bit Raspberry Pi OS (arm64)
cross build --release --target arm-unknown-linux-gnueabihf  # 32-bit Raspberry Pi OS (armhf, also Pi Zero / Pi 1)
```

The binary ends up in `target/<target>/release/beatnik-hardware-api`.

## 5. Install the Build as a Service on a Pi

Copy the binary and the bundled configs to the Pi, for example into `~/beatnik-hardware-api`:

```bash
scp target/aarch64-unknown-linux-gnu/release/beatnik-hardware-api beatnik@<pi-hostname>:~/beatnik-hardware-api/
scp -r camilladsp beatnik-hardware.service beatnik@<pi-hostname>:~/beatnik-hardware-api/
```

Then, on the Pi:

```bash
cd ~/beatnik-hardware-api
sudo cp beatnik-hardware.service /etc/systemd/system/

# IMPORTANT: Edit the service file so WorkingDirectory points to ~/beatnik-hardware-api
# and ExecStart to ~/beatnik-hardware-api/beatnik-hardware-api (use absolute paths)
sudo nano /etc/systemd/system/beatnik-hardware.service

sudo systemctl daemon-reload
sudo systemctl enable beatnik-hardware.service
sudo systemctl start beatnik-hardware.service
```

The service runs as `root` because it writes to `/boot/firmware/config.txt`. Follow the logs with:
```bash
sudo journalctl -u beatnik-hardware.service -f
```

> **Note:** Running `./setup.sh` replaces your build with the latest GitHub release. Don't use it (or the `/api/update/api` route) on a development install.

## 6. Releases

Releases are built by GitHub Actions (`.github/workflows/release.yml`) whenever a tag starting with `v` is pushed. The workflow runs the tests, cross-compiles for `arm64` and `armhf`, and attaches per architecture:

- `beatnik-hardware-api-<arch>.tar.gz`: binary, `camilladsp/`, service files and `setup.sh`. This is the artifact `setup.sh` installs.
- `beatnik-hardware-api_<version>_<arch>.deb`: binary and `camilladsp/` in `/opt/beatnik-hardware-api`, systemd units in `/etc/systemd/system`, service enabled and started by `postinst`. See [INSTALL_DEB.md](INSTALL_DEB.md).

To publish a release (`cargo release patch --execute`, then push), see the [Tagging & Releasing Guide](RELEASING.md).

Devices pick up the new release the next time they run `setup.sh` (or the update route).

## Uninstalling

See the [Uninstall Guide](UNINSTALL.md).

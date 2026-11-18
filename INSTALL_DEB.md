# Beatnik Hardware API: Debian Package Install

Each GitHub release ships `beatnik-hardware-api_<version>_<arch>.deb` packages for `arm64` (64-bit Raspberry Pi OS) and `armhf` (32-bit). The package installs the API (a single binary), registers the systemd service, and starts it. No Node.js or `setup.sh` is needed on the Pi.

Related guides: [Updating](UPDATING_DEB.md) · [Uninstalling](UNINSTALL_DEB.md) · [Script-based install](INSTALL.md)

## What the package installs

| Path | Content |
|------|---------|
| `/opt/beatnik-hardware-api/beatnik-hardware-api` | The API binary |
| `/opt/beatnik-hardware-api/camilladsp/` | Bundled CamillaDSP configs; copied on start to `/var/lib/beatnik-hardware-api/camilladsp` (existing files are kept) |
| `/etc/systemd/system/beatnik-hardware.service` | systemd unit (runs `/opt/beatnik-hardware-api/beatnik-hardware-api` as root) |
| `/etc/systemd/system/camilladsp.service` | CamillaDSP unit, using `/usr/local/bin/camilladsp` and the config in `/var/lib/beatnik-hardware-api/camilladsp/configs/client_config.yml` |
| `/etc/systemd/system/camilladsp-preamp.service` | Preamp CamillaDSP unit (port 1235), using the config in `/var/lib/beatnik-hardware-api/camilladsp/preamp/client_config.yml`; restarted by the preamp config API |

CamillaDSP itself must be installed at `/usr/local/bin/camilladsp`; the package only provides its service and configs. Without the binary, `camilladsp.service` keeps restarting but the API still works.

## Prerequisites

- Raspberry Pi OS (Bookworm or newer recommended) with `sudo` access.

Check your architecture (picks the `.deb` to download):
```bash
dpkg --print-architecture   # arm64 or armhf
```

## Install

1. Download the latest `.deb` from the [releases page](https://github.com/byrdsandbytes/beatnik-hardware-api/releases/latest), for example:
   ```bash
   VERSION=0.9.0   # replace with the latest version
   ARCH=$(dpkg --print-architecture)
   wget https://github.com/byrdsandbytes/beatnik-hardware-api/releases/download/v${VERSION}/beatnik-hardware-api_${VERSION}_${ARCH}.deb
   ```
2. Install it with `apt`:
   ```bash
   sudo apt-get install ./beatnik-hardware-api_${VERSION}_${ARCH}.deb
   ```

The post-install script reloads systemd, enables the service on boot, and starts it.

## Coming from the script install?

The script install (`setup.sh`) and the package use the same service name. Remove the old install first so the two do not conflict:

```bash
sudo systemctl stop beatnik-hardware.service
rm -rf ~/beatnik-hardware-api
```

Then install the package as above; it replaces the old service file.

## Verify

```bash
sudo systemctl status beatnik-hardware.service
curl http://localhost:3000/api/hardware/status
```

Logs:
```bash
journalctl -u beatnik-hardware.service -f
```

## Notes

- The in-app update route (`POST /api/update/api`) downloads and installs the latest `.deb` for this architecture. You can also update manually with the steps in the [Updating guide](UPDATING_DEB.md).
- Upgrading from the former Node.js package (`_all.deb`) works the same way: apt replaces it, and the old `dist/` and `node_modules/` are removed. Node.js itself stays installed.
- Edit the service (for example to set `CONFIG_PATH`) with `sudo systemctl edit beatnik-hardware.service`; overrides survive package upgrades.

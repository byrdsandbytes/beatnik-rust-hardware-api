# Beatnik Hardware API: Updating the Debian Package

Related guides: [Install](INSTALL_DEB.md) · [Uninstall](UNINSTALL_DEB.md)

## Update

1. Check the installed version:
   ```bash
   dpkg-query -W beatnik-hardware-api
   ```
2. Download the new `.deb` from the [releases page](https://github.com/byrdsandbytes/beatnik-hardware-api/releases/latest):
   ```bash
   VERSION=0.9.0   # replace with the new version
   ARCH=$(dpkg --print-architecture)   # arm64 or armhf
   wget https://github.com/byrdsandbytes/beatnik-hardware-api/releases/download/v${VERSION}/beatnik-hardware-api_${VERSION}_${ARCH}.deb
   ```
3. Install it over the existing version:
   ```bash
   sudo apt-get install ./beatnik-hardware-api_${VERSION}_${ARCH}.deb
   ```

dpkg replaces the files in `/opt/beatnik-hardware-api`, and the post-install script reloads systemd and restarts the service. No manual stop or start is needed.

## Verify

```bash
dpkg-query -W beatnik-hardware-api
sudo systemctl status beatnik-hardware.service
curl http://localhost:3000/api/hardware/status
```

## Your service file changes

The service file in `/etc/systemd/system` is treated by dpkg as a configuration file. If you edited it directly, dpkg asks whether to keep your version or install the new one. To avoid this, put customisations in a drop-in instead:

```bash
sudo systemctl edit beatnik-hardware.service
```

To skip the prompt and keep your edited file during an update, use:
```bash
sudo apt-get install -o Dpkg::Options::="--force-confold" ./beatnik-hardware-api_${VERSION}_${ARCH}.deb
```

## Downgrade

Install an older `.deb` the same way. Add `--allow-downgrades`:
```bash
sudo apt-get install --allow-downgrades ./beatnik-hardware-api_<older-version>_<arch>.deb
```
Versions up to 0.8.x are the Node.js implementation (`_all.deb`) and need Node.js 22 at `/usr/bin/node`:
```bash
sudo apt-get install --allow-downgrades ./beatnik-hardware-api_0.8.0_all.deb
```

## Notes

- Updates are manual; the package does not add an apt repository.
- The in-app update route (`POST /api/update/api`) performs the same steps automatically: it downloads the latest `.deb` from GitHub and installs it with `apt-get` (keeping edited service files). It requires the package install; for the script install use `./setup.sh`.

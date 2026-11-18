# Beatnik Hardware API: Installation & Updating Guide

This guide covers the necessary steps to install, configure, and maintain the Beatnik Hardware API on your Raspberry Pi. 

There are two primary ways to install the service. **Method 1 (Production Release)** is the most robust and professional choice, as it keeps your Raspberry Pi free of development dependencies and compiling overhead. **Method 2 (Source Script)** is better suited for active development.

---

## Prerequisites

Before you begin, ensure your Raspberry Pi meets the following requirements:
- **OS:** Raspberry Pi OS (Bookworm or newer recommended)
- **Architecture:** 64-bit (`arm64`) or 32-bit (`armhf`) Raspberry Pi OS.
- **Privileges:** Root access (`sudo`) is required to modify system configuration files and manage systemd services.

The API is a single self-contained binary; no Node.js or other runtime is needed.

---

## Installation

### Method 1: Production Setup Script (Recommended)
We provide an automated setup script that downloads the latest pre-built release artifact and configures the systemd service. This is the cleanest way to run the service in production.

```bash
mkdir -p ~/beatnik-hardware-api
cd ~/beatnik-hardware-api
wget https://raw.githubusercontent.com/byrdsandbytes/beatnik-hardware-api/master/setup.sh
chmod +x setup.sh
./setup.sh
```

**What the script does:**
1. Creates a `beatnik-hardware-api` directory in your home folder (`~/beatnik-hardware-api`).
2. Downloads and extracts the latest release artifact for your architecture (`beatnik-hardware-api-<arch>.tar.gz`) from GitHub.
3. Automatically patches and installs `beatnik-hardware.service` to systemd and starts it.

---

### Method 2: Manual Source Installation (Development)
If you want to build the project directly from source on the Pi itself:

**1. Clone Repository**
```bash
mkdir -p ~/beatnik-hardware-api
cd ~/beatnik-hardware-api
git clone https://github.com/byrdsandbytes/beatnik-hardware-api.git .
```

**2. Build**

Install Rust via [rustup](https://rustup.rs) if needed, then build (this takes a while on a Pi; see [BUILD_FROM_SOURCE.md](BUILD_FROM_SOURCE.md#cross-compiling-for-the-pi) for cross-compiling on another machine):
```bash
cargo build --release
cp target/release/beatnik-hardware-api .
```

**3. Set up the System Service**
```bash
# Copy the service file
sudo cp beatnik-hardware.service /etc/systemd/system/

# IMPORTANT: Edit the service file so WorkingDirectory points to ~/beatnik-hardware-api
# and ExecStart to ~/beatnik-hardware-api/beatnik-hardware-api (use absolute paths)
sudo nano /etc/systemd/system/beatnik-hardware.service

# Enable and start the service
sudo systemctl daemon-reload
sudo systemctl enable beatnik-hardware.service
sudo systemctl start beatnik-hardware.service
```

---

## Updating the Service

To make future updates completely seamless, the `setup.sh` script is designed to also act as an updater. 

Whenever a new version (`vX.X.X`) is tagged and pushed to GitHub, an automated CI/CD pipeline (GitHub Actions) instantly builds a new release artifact. You can easily pull this update onto your Raspberry Pi by running the local setup script again:

```bash
cd ~/beatnik-hardware-api
./setup.sh
```

**What the updater does:**
1. Checks if the service is currently running, and smoothly stops it.
2. Queries the GitHub API for the latest release of `byrdsandbytes/beatnik-hardware-api`.
3. Downloads & extracts the new `beatnik-hardware-api-<arch>.tar.gz` artifact over the existing installation (leftovers of the former Node.js version, `dist/` and `node_modules/`, are removed).
4. Restarts the system service and cleans up.

> **Upgrading from the Node.js version (0.8.x and older):** the old `setup.sh` looks for a release artifact that no longer exists. Download the new script once before running it:
> ```bash
> cd ~/beatnik-hardware-api
> wget -O setup.sh https://raw.githubusercontent.com/byrdsandbytes/beatnik-hardware-api/master/setup.sh
> chmod +x setup.sh && ./setup.sh
> ```



---

## Checking Service Status

Regardless of which method you chose, you can always check if the API is running smoothly via:
```bash
sudo systemctl status beatnik-hardware.service
```

You can also test the hardware status endpoint:
```bash
curl http://localhost:3000/api/hardware/status
```
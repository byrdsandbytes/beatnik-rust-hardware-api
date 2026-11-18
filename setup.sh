#!/bin/bash

# Exit on error
set -e

echo "==> Starting Beatnik Hardware API Production Setup..."

REPO="byrdsandbytes/beatnik-hardware-api"
INSTALL_DIR="$HOME/beatnik-hardware-api"
SERVICE_FILE="beatnik-hardware.service"
BINARY="beatnik-hardware-api"

# Release tarballs exist for arm64 (64-bit Raspberry Pi OS) and armhf (32-bit)
ARCH=$(dpkg --print-architecture)
case "$ARCH" in
  arm64|armhf) ;;
  *)
    echo "!! Unsupported architecture: $ARCH (supported: arm64, armhf)"
    exit 1
    ;;
esac
ARTIFACT="beatnik-hardware-api-${ARCH}.tar.gz"

if systemctl is-active --quiet "$SERVICE_FILE"; then
    echo "-- Stopping existing service for update..."
    sudo systemctl stop "$SERVICE_FILE"
fi

echo "-- Creating installation directory at $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

echo "-- Finding latest release on GitHub..."
DOWNLOAD_URL=$(curl -s https://api.github.com/repos/$REPO/releases/latest | grep "browser_download_url" | grep "$ARTIFACT" | cut -d '"' -f 4)

if [ -z "$DOWNLOAD_URL" ]; then
  echo "!! Could not find the latest release. Check your internet connection or repository name."
  exit 1
fi

echo "-- Downloading release ($ARCH)..."
wget -q -O "$ARTIFACT" "$DOWNLOAD_URL"

echo "-- Extracting release..."
# Leftovers of the former Node.js version
rm -rf dist node_modules package.json package-lock.json
tar -xzvf "$ARTIFACT"
rm "$ARTIFACT"
chmod +x "$BINARY"

# Configure Systemd Service
echo "==> Configuring Systemd Service..."

TEMP_SERVICE_FILE="${SERVICE_FILE}.tmp"
cp "$SERVICE_FILE" "$TEMP_SERVICE_FILE"

# Point WorkingDirectory and ExecStart to the install directory (escape slashes for sed)
ESCAPED_INSTALL_DIR=$(echo "$INSTALL_DIR" | sed 's/\//\\\//g')
sed -i "s/^WorkingDirectory=.*/WorkingDirectory=$ESCAPED_INSTALL_DIR/" "$TEMP_SERVICE_FILE"
sed -i "s/^ExecStart=.*/ExecStart=$ESCAPED_INSTALL_DIR\/$BINARY/" "$TEMP_SERVICE_FILE"

echo "   Service file patched with local paths."

# Install Service (requires sudo)
echo "==> Installing Service to /etc/systemd/system/ (requires sudo)..."
sudo cp "$TEMP_SERVICE_FILE" "/etc/systemd/system/$SERVICE_FILE"
rm "$TEMP_SERVICE_FILE"

echo "   Reloading systemd daemon..."
sudo systemctl daemon-reload

echo "   Enabling service..."
sudo systemctl enable "$SERVICE_FILE"

echo "   Restarting service..."
sudo systemctl restart "$SERVICE_FILE"

echo "==> Setup Complete!"
echo "   Checking service status..."
sudo systemctl status "$SERVICE_FILE" --no-pager

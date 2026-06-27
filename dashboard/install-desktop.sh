#!/usr/bin/env bash
# Install the Pith Dashboard .desktop entry + icon into the user's local data dir
# so Wayland/X11 show the app's icon in the app-bar / taskbar (matched by app-id
# "pith-dashboard"). Re-run after building to point Exec at the current binary.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"

bin="$here/build/pith-dashboard"
[[ -x "$bin" ]] || { echo "build the app first: cmake --build build"; exit 1; }

apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/128x128/apps"
mkdir -p "$apps" "$icons"

install -m644 "$here/icon.png" "$icons/pith-dashboard.png"

# Desktop entry with an absolute Exec so launchers can start it directly.
sed "s|^Exec=.*|Exec=$bin|" "$here/pith-dashboard.desktop" > "$apps/pith-dashboard.desktop"
chmod 644 "$apps/pith-dashboard.desktop"

update-desktop-database "$apps" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo "Installed pith-dashboard.desktop -> $apps"
echo "Installed icon -> $icons/pith-dashboard.png"
echo "If the old icon sticks, log out/in or restart the shell/compositor."

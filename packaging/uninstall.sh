#!/usr/bin/env bash
# Remove the locally-installed hexforge application.
set -euo pipefail

rm -fv "$HOME/.local/bin/hexforge"
rm -fv "$HOME/.local/share/applications/hexforge.desktop"
rm -fv "$HOME/.local/share/icons/hicolor/scalable/apps/hexforge.svg"

update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo "✅ Uninstalled hexforge."

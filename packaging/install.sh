#!/usr/bin/env bash
# Install hexforge as a local GNOME/XDG application (no root required).
# Reversible via packaging/uninstall.sh.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

bin_dir="$HOME/.local/bin"
app_dir="$HOME/.local/share/applications"
icon_dir="$HOME/.local/share/icons/hicolor/scalable/apps"

echo "==> Building release binary…"
cargo build --release --bin hexforge

echo "==> Installing binary → $bin_dir/hexforge"
install -Dm755 target/release/hexforge "$bin_dir/hexforge"

echo "==> Installing icon → $icon_dir/hexforge.svg"
install -Dm644 packaging/hexforge.svg "$icon_dir/hexforge.svg"

echo "==> Installing desktop entry → $app_dir/hexforge.desktop"
mkdir -p "$app_dir"
# Use an absolute Exec — a GNOME-launched .desktop does not always inherit
# ~/.local/bin in PATH.
sed "s|^Exec=.*|Exec=$bin_dir/hexforge|" packaging/hexforge.desktop \
    > "$app_dir/hexforge.desktop"
chmod 644 "$app_dir/hexforge.desktop"

echo "==> Refreshing desktop/icon caches"
update-desktop-database "$app_dir" 2>/dev/null || true
gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

echo "✅ Installed. Launch \"hexforge\" from the GNOME app grid / search."
echo "   (Ensure $bin_dir is on PATH for terminal use.)"

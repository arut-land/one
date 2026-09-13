#!/bin/sh
# Install desktop integration into an XDG data directory, without touching binaries.
set -eu
data_dir=${1:-${XDG_DATA_HOME:-$HOME/.local/share}}
source_dir=$(CDPATH= cd -- "$(dirname -- "$0")/data" && pwd)
install -Dm644 "$source_dir/dev.arut.Arut.desktop" "$data_dir/applications/dev.arut.Arut.desktop"
for size in 16 24 32 48 64 128 256 512; do
    relative="icons/hicolor/${size}x${size}/apps/dev.arut.Arut.png"
    install -Dm644 "$source_dir/$relative" "$data_dir/$relative"
done
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache --force --ignore-theme-index "$data_dir/icons/hicolor"
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data_dir/applications"
fi

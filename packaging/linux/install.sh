#!/usr/bin/env bash
# Installs Hymn Book for the current user (no root needed).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bin="$HOME/.local/bin"; apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor/512x512/apps"
mkdir -p "$bin" "$apps" "$icons"
install -m 755 "$here/hymnbook" "$bin/hymnbook"
install -m 644 "$here/hymnbook.png" "$icons/hymnbook.png"
install -m 644 "$here/hymnbook.desktop" "$apps/hymnbook.desktop"
echo "Installed. Find 'Hymn Book' in your applications menu, or run: hymnbook"
echo "(Make sure $bin is on your PATH.)"

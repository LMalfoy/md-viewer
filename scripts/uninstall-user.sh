#!/bin/sh
set -eu

HOME_DIR=${HOME:?HOME is required for a user-local installation}
BIN_DIR=${XDG_BIN_HOME:-"$HOME_DIR/.local/bin"}
DATA_DIR=${XDG_DATA_HOME:-"$HOME_DIR/.local/share"}

rm -f "$BIN_DIR/md-viewer"
rm -f "$DATA_DIR/applications/md-viewer.desktop"
rm -f "$DATA_DIR/icons/hicolor/scalable/apps/md-viewer.svg"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DATA_DIR/applications"
fi

echo "MD Viewer was removed from the user-local installation."

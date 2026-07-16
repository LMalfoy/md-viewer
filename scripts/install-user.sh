#!/bin/sh
set -eu

PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
BINARY="$PROJECT_ROOT/target/release/md-viewer"
SET_DEFAULT=true

if [ "${1:-}" = "--no-default" ]; then
    SET_DEFAULT=false
elif [ -n "${1:-}" ]; then
    echo "Usage: $0 [--no-default]" >&2
    exit 2
fi

if [ ! -x "$BINARY" ]; then
    echo "Release binary not found: $BINARY" >&2
    echo "Run: cargo build --release" >&2
    exit 1
fi

HOME_DIR=${HOME:?HOME is required for a user-local installation}
BIN_DIR=${XDG_BIN_HOME:-"$HOME_DIR/.local/bin"}
DATA_DIR=${XDG_DATA_HOME:-"$HOME_DIR/.local/share"}
INSTALLED_BINARY="$BIN_DIR/md-viewer"
DESKTOP_FILE="$DATA_DIR/applications/md-viewer.desktop"
ICON_FILE="$DATA_DIR/icons/hicolor/scalable/apps/md-viewer.svg"

install -D -m 0755 "$BINARY" "$INSTALLED_BINARY"
install -D -m 0644 "$PROJECT_ROOT/assets/md-viewer.svg" "$ICON_FILE"

TEMP_DESKTOP=$(mktemp)
trap 'rm -f "$TEMP_DESKTOP"' EXIT INT TERM
sed "s|@BINARY@|$INSTALLED_BINARY|g" \
    "$PROJECT_ROOT/packaging/md-viewer.desktop.in" >"$TEMP_DESKTOP"
install -D -m 0644 "$TEMP_DESKTOP" "$DESKTOP_FILE"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$DATA_DIR/applications"
fi

if [ "$SET_DEFAULT" = true ] && command -v xdg-mime >/dev/null 2>&1; then
    xdg-mime default md-viewer.desktop text/markdown
    xdg-mime default md-viewer.desktop text/x-markdown
fi

echo "Installed MD Viewer:"
echo "  binary:  $INSTALLED_BINARY"
echo "  desktop: $DESKTOP_FILE"
echo "  icon:    $ICON_FILE"

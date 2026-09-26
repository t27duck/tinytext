#!/usr/bin/env bash
# Install (or with --uninstall, remove) tinytext for the current user.
# Set PREFIX to install elsewhere, e.g. PREFIX=/usr/local sudo -E scripts/install.sh
set -euo pipefail

cd "$(dirname "$0")/.."
PREFIX="${PREFIX:-$HOME/.local}"
BIN="$PREFIX/bin/tinytext"
DESKTOP="$PREFIX/share/applications/dev.tinytext.TinyText.desktop"

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$BIN" "$DESKTOP"
    echo "Removed $BIN and $DESKTOP"
else
    if [ ! -x dist/tinytext ]; then
        echo "dist/tinytext not found; run scripts/build.sh first" >&2
        exit 1
    fi
    install -Dm755 dist/tinytext "$BIN"
    mkdir -p "$(dirname "$DESKTOP")"
    sed "s|^Exec=tinytext|Exec=$BIN|" packaging/dev.tinytext.TinyText.desktop > "$DESKTOP"
    chmod 644 "$DESKTOP"
    echo "Installed $BIN and $DESKTOP"
fi

if command -v update-desktop-database >/dev/null; then
    update-desktop-database -q "$(dirname "$DESKTOP")" || true
fi

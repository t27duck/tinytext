#!/usr/bin/env bash
# Install (or with --uninstall, remove) tinytext for the current user.
#
# Options:
#   --set-default   also make tinytext the default app for the file types in
#                   its desktop entry (plain text and Markdown)
#   --uninstall     remove tinytext and any defaults pointing at it
#
# Set PREFIX to install elsewhere, e.g. PREFIX=/usr/local sudo -E scripts/install.sh
set -euo pipefail

cd "$(dirname "$0")/.."
PREFIX="${PREFIX:-$HOME/.local}"
DESKTOP_ID=dev.tinytext.TinyText.desktop
BIN="$PREFIX/bin/tinytext"
DESKTOP="$PREFIX/share/applications/$DESKTOP_ID"
LICENSES="$PREFIX/share/licenses/tinytext"
MIMEAPPS="${XDG_CONFIG_HOME:-$HOME/.config}/mimeapps.list"

uninstall=false
set_default=false
for arg in "$@"; do
    case "$arg" in
        --uninstall) uninstall=true ;;
        --set-default) set_default=true ;;
        *) echo "Unknown option: $arg" >&2; exit 1 ;;
    esac
done

if $uninstall; then
    rm -f "$BIN" "$DESKTOP"
    rm -rf "$LICENSES"
    echo "Removed $BIN, $DESKTOP and $LICENSES"
    if [ -f "$MIMEAPPS" ] && grep -q "=$DESKTOP_ID" "$MIMEAPPS"; then
        sed -i "/=$DESKTOP_ID;*\$/d" "$MIMEAPPS"
        echo "Removed tinytext defaults from $MIMEAPPS"
    fi
else
    if [ ! -x dist/tinytext ]; then
        echo "dist/tinytext not found; run scripts/build.sh first" >&2
        exit 1
    fi
    install -Dm755 dist/tinytext "$BIN"
    mkdir -p "$(dirname "$DESKTOP")"
    sed "s|^Exec=tinytext|Exec=$BIN|" packaging/$DESKTOP_ID > "$DESKTOP"
    chmod 644 "$DESKTOP"
    install -Dm644 -t "$LICENSES" dist/LICENSE dist/THIRD-PARTY-LICENSES
    echo "Installed $BIN, $DESKTOP and $LICENSES"
fi

if command -v update-desktop-database >/dev/null; then
    update-desktop-database -q "$(dirname "$DESKTOP")" || true
fi
# KDE Plasma reads the application menu from its sycoca cache.
if command -v kbuildsycoca6 >/dev/null; then
    kbuildsycoca6 >/dev/null 2>&1 || true
fi

if ! $uninstall && $set_default; then
    if ! command -v xdg-mime >/dev/null; then
        echo "xdg-mime not found (install xdg-utils); skipping --set-default" >&2
        exit 1
    fi
    read -ra types <<< "$(sed -n 's/^MimeType=//p' packaging/$DESKTOP_ID | tr ';' ' ')"
    xdg-mime default "$DESKTOP_ID" "${types[@]}"
    echo "tinytext is now the default for: ${types[*]}"
fi

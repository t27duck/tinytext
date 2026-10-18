#!/usr/bin/env bash
# Package the release build in ./dist as a tarball in target/package.
# Run scripts/build.sh first. The tarball mirrors the repo layout, so the
# unpacked directory can be installed with its own scripts/install.sh.
# Usage: scripts/package.sh [tag]   (fails if tag is not v<Cargo.toml version>)
set -euo pipefail

cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n1)

if [ $# -gt 0 ] && [ "$1" != "v$VERSION" ]; then
    echo "Tag $1 does not match Cargo.toml version $VERSION" >&2
    exit 1
fi
for f in dist/tinytext dist/LICENSE dist/THIRD-PARTY-LICENSES; do
    if [ ! -f "$f" ]; then
        echo "$f not found; run scripts/build.sh first" >&2
        exit 1
    fi
done

NAME="tinytext-$VERSION-$(uname -m)-linux"
OUT=target/package
STAGE="$OUT/$NAME"
rm -rf "$STAGE"
mkdir -p "$STAGE/scripts"
cp -r dist packaging README.md LICENSE "$STAGE/"
cp scripts/install.sh "$STAGE/scripts/"

tar -C "$OUT" -czf "$OUT/$NAME.tar.gz" "$NAME"
(cd "$OUT" && sha256sum "$NAME.tar.gz" > "$NAME.tar.gz.sha256")
echo "Packaged $OUT/$NAME.tar.gz"

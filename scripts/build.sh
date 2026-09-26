#!/usr/bin/env bash
# Build tinytext inside Docker and place the binary in ./dist.
# Usage: scripts/build.sh [cargo subcommand and args]   (default: build --release)
set -euo pipefail

cd "$(dirname "$0")/.."
IMAGE=tinytext-build

docker build -q -t "$IMAGE" . >/dev/null

args=("$@")
[ ${#args[@]} -eq 0 ] && args=(build --release)

docker run --rm \
    -u "$(id -u):$(id -g)" \
    -e CARGO_HOME=/src/target/.cargo-home \
    -v "$PWD:/src" \
    "$IMAGE" cargo "${args[@]}"

if [ "${args[0]}" = build ] && [[ " ${args[*]} " == *" --release "* ]]; then
    mkdir -p dist
    cp target/release/tinytext dist/tinytext
    cp LICENSE dist/LICENSE
    docker run --rm \
        -u "$(id -u):$(id -g)" \
        -e CARGO_HOME=/src/target/.cargo-home \
        -v "$PWD:/src" \
        "$IMAGE" cargo about generate --fail -o dist/THIRD-PARTY-LICENSES about.hbs
    echo "Built dist/tinytext (with LICENSE and THIRD-PARTY-LICENSES)"
fi

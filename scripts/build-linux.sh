#!/bin/sh
# Build, test, and package the one supported native build target.
set -eu
cd "$(dirname "$0")/.."
root=$(pwd -P)
target=x86_64-unknown-linux-gnu
export CARGO_TARGET_DIR="$root/target"
binary_dir="$CARGO_TARGET_DIR/$target/release"
binary="$binary_dir/herdr-crossroads"
output="$CARGO_TARGET_DIR/builds"
version=$(python3 scripts/check-version.py)
archive="herdr-crossroads-$version-$target.tar.gz"
stem="$output/${archive%.tar.gz}"
mkdir -p "$output"
trap 'status=$?; if [ "$status" -ne 0 ]; then rm -f "$output/$archive" "$output/$archive.sha256" "$stem.provenance.json" "$stem.link.map"; fi; rm -f "$output/$archive.tmp" "$output/$archive.sha256.tmp"; exit "$status"' 0
trap 'exit 130' INT
trap 'exit 143' TERM
rm -f "$output/$archive" "$output/$archive.sha256" \
    "$stem.provenance.json" "$stem.link.map" \
    "$output/$archive.tmp" "$output/$archive.sha256.tmp"

if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ]; then
    printf '%s\n' 'Only native Linux x86_64 builds are supported.' >&2
    exit 1
fi

python3 scripts/third-party-notices.py --check
# Never let a stale binary stand in for this build's output.
rm -f "$binary" "$binary.sha256"
python3 scripts/build-provenance.py "$target" "$binary" "$stem"
test -f "$binary"
test -x "$binary"
for gate in latency render keymaps hints; do
    python3 "tests/pty_$gate.py" "$binary"
done

(
    cd "$binary_dir"
    sha256sum herdr-crossroads > herdr-crossroads.sha256
    tar --owner=0 --group=0 --numeric-owner -czf "$output/$archive.tmp" \
        herdr-crossroads herdr-crossroads.sha256 -C "$root" LICENSE THIRD-PARTY-NOTICES.html
)
mv "$output/$archive.tmp" "$output/$archive"
(
    cd "$output"
    sha256sum "$archive" > "$archive.sha256.tmp"
    mv "$archive.sha256.tmp" "$archive.sha256"
)
printf 'Built and passed all four full PTYs: target/builds/%s\n' "$archive"
printf 'Archive checksum: target/builds/%s.sha256\n' "$archive"
printf 'Builder evidence (not distribution approval): target/builds/%s.provenance.json and target/builds/%s.link.map\n' "${archive%.tar.gz}" "${archive%.tar.gz}"

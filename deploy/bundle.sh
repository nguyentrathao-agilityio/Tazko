#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=${TARGET:-x86_64-unknown-linux-musl}
OUT=${OUT:-dist}
BINARY=${BINARY:-}

if [ -z "$BINARY" ]; then
  cargo build --release --locked --target "$TARGET"
  BINARY="target/$TARGET/release/tazko"
fi

commit=$(git rev-parse --short=12 HEAD)
stamp=$(date -u +%Y%m%d-%H%M%S)
version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2) $commit"
if [ -n "$(git status --porcelain)" ]; then
  version="$version (uncommitted changes)"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
cp "$BINARY" "$tmp/tazko"
chmod 0755 "$tmp/tazko"
printf '%s\n' "$version" >"$tmp/VERSION"
mkdir -p "$OUT"
bundle="$OUT/tazko-$stamp-$commit.tar.gz"
tar -czf "$bundle" -C "$tmp" tazko VERSION
echo "$bundle"

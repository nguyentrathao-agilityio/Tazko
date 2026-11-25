#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET=${TARGET:-x86_64-unknown-linux-musl}
OUT=${OUT:-dist}
BINARY=${BINARY:-}

if [ -n "${TAZKO_COMMIT:-}" ]; then
  commit=$TAZKO_COMMIT
  dirty=
else
  commit=$(git rev-parse HEAD | cut -c1-12)
  dirty=$(git status --porcelain)
fi
if ! [[ "$commit" =~ ^[0-9a-f]{12}$ ]]; then
  echo "bundle.sh: no commit id (set TAZKO_COMMIT to 12 hex characters outside a repository)" >&2
  exit 1
fi

if [ -z "$BINARY" ]; then
  cargo build --release --locked --target "$TARGET"
  BINARY="${CARGO_TARGET_DIR:-target}/$TARGET/release/tazko"
fi

stamp=$(date -u +%Y%m%d-%H%M%S)
version="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2) $commit"
if [ -n "$dirty" ]; then
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

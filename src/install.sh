#!/bin/sh
set -eu
host="__TAZKO_HOST__"
target="__TAZKO_TARGET__"
os=$(uname -s | tr '[:upper:]' '[:lower:]')
arch=$(uname -m)
case "$os" in darwin) os=macos ;; esac
case "$arch" in arm64) arch=aarch64 ;; amd64) arch=x86_64 ;; esac
if [ "$os-$arch" != "$target" ]; then
  echo "This server offers the command line tool for $target only; this computer is $os-$arch." >&2
  echo "Build it from source with: cargo install --git <repository> tazko" >&2
  exit 1
fi
dir="${TAZKO_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$dir"
tmp=$(mktemp)
curl -fsSL "$host/cli/download" -o "$tmp"
chmod 755 "$tmp"
mv "$tmp" "$dir/tazko"
echo "Installed tazko in $dir"
case ":$PATH:" in *":$dir:"*) ;; *) echo "Add $dir to your PATH to run tazko from anywhere." ;; esac
echo "Next: tazko auth login --host $host"

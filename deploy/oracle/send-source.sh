#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."

TO=${1:-}
if [ -z "$TO" ]; then
  echo "usage: deploy/oracle/send-source.sh <user>@<vm address>" >&2
  exit 2
fi

if [ -n "$(git status --porcelain)" ]; then
  echo "send-source.sh: commit, stash or remove your changes (untracked files too) first; only committed code is sent" >&2
  exit 1
fi

commit=$(git rev-parse HEAD | cut -c1-12)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/tazko"
git archive --format=tar HEAD | tar -x -C "$tmp/tazko"
pnpm -C "$tmp/tazko/web" install --frozen-lockfile
pnpm -C "$tmp/tazko/web" build
rm -rf "$tmp/tazko/web/node_modules"
printf '%s\n' "$commit" >"$tmp/tazko/COMMIT"
tar -czf "$tmp/tazko-src-$commit.tar.gz" -C "$tmp" tazko
scp "$tmp/tazko-src-$commit.tar.gz" "$TO:"
echo "sent tazko-src-$commit.tar.gz to $TO"

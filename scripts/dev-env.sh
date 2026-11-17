#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

TOOLS=(cargo rustc rustfmt cargo-clippy cargo-deny node npm pnpm sqlite3)
MIN_NODE_MAJOR=22

usage() {
  echo "Usage: scripts/dev-env.sh check|init|versions"
  echo "  check     tools are on PATH, Node is LTS ${MIN_NODE_MAJOR}+, .env.local is git-ignored"
  echo "  init      create .env.local from .env.example if it does not exist"
  echo "  versions  print the tool versions"
}

check() {
  local bad=0

  for t in "${TOOLS[@]}"; do
    if ! command -v "$t" >/dev/null 2>&1; then
      echo "  FAIL $t is not on PATH (see docs/dev.md)"
      bad=1
    fi
  done

  if command -v node >/dev/null 2>&1; then
    local major
    major=$(node -p 'process.versions.node.split(".")[0]')
    if [ "$major" -lt "$MIN_NODE_MAJOR" ] || [ $((major % 2)) -ne 0 ]; then
      echo "  FAIL node $major is not an LTS line >= $MIN_NODE_MAJOR"
      bad=1
    fi
  fi

  if ! git check-ignore -q .env.local; then
    echo "  FAIL .env.local is not git-ignored"
    bad=1
  fi

  if git ls-files --error-unmatch .env.local >/dev/null 2>&1; then
    echo "  FAIL .env.local is tracked"
    bad=1
  fi

  if [ "$bad" -eq 0 ]; then
    echo "  local dev target: ok"
  fi

  return "$bad"
}

init() {
  if [ -f .env.local ]; then
    echo ".env.local already exists; left unchanged"
    return 0
  fi

  cp .env.example .env.local
  chmod 600 .env.local
  echo "created .env.local; fill in TAZKO_GOOGLE_CLIENT_ID and TAZKO_GOOGLE_CLIENT_SECRET (docs/dev.md)"
}

versions() {
  for t in "${TOOLS[@]}"; do
    if command -v "$t" >/dev/null 2>&1; then
      case "$t" in
        sqlite3) printf '%-13s %s\n' "$t" "$(sqlite3 --version | cut -d' ' -f1)" ;;
        *) printf '%-13s %s\n' "$t" "$("$t" --version 2>/dev/null | head -1)" ;;
      esac
    else
      printf '%-13s missing\n' "$t"
    fi
  done
}

case "${1:-}" in
  check) check ;;
  init) init ;;
  versions) versions ;;
  *) usage; exit 2 ;;
esac

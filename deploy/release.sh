#!/usr/bin/env bash
set -euo pipefail

ROOT=${TAZKO_ROOT:-/opt/tazko}
KEEP=${TAZKO_KEEP:-5}
WAIT=${TAZKO_HEALTH_WAIT:-20}
HEALTH=${TAZKO_HEALTH_URL:-http://127.0.0.1:8791/health}
RESTART=${TAZKO_RESTART:-sudo -n /usr/bin/systemctl restart tazko.service}
RELEASES="$ROOT/releases"
CURRENT="$ROOT/current"

usage() {
  echo "usage: tazko-release <tazko-<time>-<commit>.tar.gz> | --rollback | --list" >&2
  exit 2
}

current_name() {
  if [ -L "$CURRENT" ]; then
    basename "$(readlink "$CURRENT")"
  fi
}

releases() {
  find "$RELEASES" -mindepth 1 -maxdepth 1 -type d ! -name '.*' -exec basename {} \; | sort
}

healthy() {
  local deadline=$((SECONDS + WAIT))
  while [ "$SECONDS" -le "$deadline" ]; do
    if curl -fsS --max-time 2 "$HEALTH" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

switch_to() {
  ln -sfn "$RELEASES/$1" "$CURRENT"
  $RESTART
}

prune() {
  local keep_now
  keep_now=$(current_name)
  releases | sort -r | tail -n +"$((KEEP + 1))" | while read -r old; do
    if [ "$old" != "$keep_now" ]; then
      rm -rf "${RELEASES:?}/$old"
      echo "removed old release $old"
    fi
  done
}

list() {
  local now
  now=$(current_name)
  releases | sort -r | while read -r r; do
    if [ "$r" = "$now" ]; then echo "* $r"; else echo "  $r"; fi
  done
}

rollback() {
  local now prev
  now=$(current_name)
  prev=$(releases | awk -v now="$now" '$0 < now' | tail -n 1)
  if [ -z "$prev" ]; then
    echo "tazko-release: no older release to roll back to" >&2
    exit 1
  fi

  switch_to "$prev"
  if healthy; then
    echo "rolled back to $prev"
  else
    echo "tazko-release: rolled back to $prev but it is not healthy either" >&2
    exit 1
  fi
}

release() {
  local bundle=$1 name tmp prev
  [ -f "$bundle" ] || { echo "tazko-release: $bundle not found" >&2; exit 1; }
  name=$(basename "$bundle" .tar.gz)
  name=${name#tazko-}
  if ! [[ "$name" =~ ^[0-9]{8}-[0-9]{4,6}-[0-9a-f]{7,40}$ ]]; then
    echo "tazko-release: bundle must be named tazko-<YYYYMMDD-HHMM[SS]>-<commit>.tar.gz" >&2
    exit 1
  fi

  if [ -e "$RELEASES/$name" ]; then
    echo "tazko-release: $name is already released; use --rollback or build a new bundle" >&2
    exit 1
  fi

  if [ "$(tar -tzf "$bundle" | sed 's#^\./##' | grep -v '^$' | sort | tr '\n' ' ')" != "VERSION tazko " ]; then
    echo "tazko-release: the bundle must contain exactly tazko and VERSION" >&2
    exit 1
  fi

  mkdir -p "$RELEASES"
  tmp="$RELEASES/.incoming-$$"
  rm -rf "$tmp"
  mkdir "$tmp"
  tar -xzf "$bundle" -C "$tmp" --no-same-owner
  chmod 0755 "$tmp/tazko"
  if ! "$tmp/tazko" --version >/dev/null 2>&1; then
    rm -rf "$tmp"
    echo "tazko-release: the new binary does not run on this machine" >&2
    exit 1
  fi
  mv "$tmp" "$RELEASES/$name"

  prev=$(current_name)
  switch_to "$name"
  if healthy; then
    echo "released $name ($(cat "$RELEASES/$name/VERSION"))"
    prune
    return 0
  fi

  echo "tazko-release: $name is not healthy after ${WAIT}s" >&2
  if [ -n "$prev" ]; then
    switch_to "$prev"
    if healthy; then
      echo "rolled back to $prev" >&2
    else
      echo "tazko-release: rolled back to $prev but it is not healthy either" >&2
    fi
  fi
  exit 1
}

case "${1:-}" in
  --list) list ;;
  --rollback) rollback ;;
  ""|-*) usage ;;
  *) release "$1" ;;
esac

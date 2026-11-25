#!/usr/bin/env bash
set -euo pipefail

FILE=${TAZKO_DUCKDNS_ENV:-${CREDENTIALS_DIRECTORY:-/etc/tazko}/tazko.env}

value() {
  { grep -E "^$1=" "$FILE" || true; } | tail -n 1 | cut -d= -f2- | tr -d '[:space:]"'"'"
}

[ -r "$FILE" ] || { echo "duckdns: cannot read $FILE" >&2; exit 2; }
domain=$(value DUCKDNS_DOMAIN | tr '[:upper:]' '[:lower:]')
token=$(value DUCKDNS_TOKEN)
domain=${domain%.duckdns.org}

if ! [[ "$domain" =~ ^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$ ]]; then
  echo "duckdns: DUCKDNS_DOMAIN must be the name before .duckdns.org" >&2
  exit 2
fi

if ! [[ "$token" =~ ^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$ ]]; then
  echo "duckdns: DUCKDNS_TOKEN is missing or not a DuckDNS token" >&2
  exit 2
fi

answer=$(printf 'url = "https://www.duckdns.org/update?domains=%s&token=%s&ip="\n' "$domain" "$token" |
  curl -fsS --max-time 20 --proto '=https' --config -)
if [ "$answer" != OK ]; then
  echo "duckdns: the update for $domain.duckdns.org was refused" >&2
  exit 1
fi

echo "duckdns: $domain.duckdns.org points at this machine"

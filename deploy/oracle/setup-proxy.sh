#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=${DRY_RUN:-0}
HERE=$(cd "$(dirname "$0")" && pwd)
HOST=${1:-}

run() {
  printf '+ %s\n' "$*"
  if [ "$DRY_RUN" != 1 ]; then
    "$@"
  fi
}

label='[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?'
if ! [[ "$HOST" =~ ^$label(\.$label)+$ ]]; then
  echo "usage: setup-proxy.sh <name>.duckdns.org (lower-case host name only)" >&2
  exit 2
fi

if [ "$DRY_RUN" != 1 ] && [ "$(id -u)" -ne 0 ]; then
  echo "setup-proxy.sh: run as root (sudo), or with DRY_RUN=1 to print the commands" >&2
  exit 2
fi

if [ "$DRY_RUN" != 1 ] && [ ! -d /etc/tazko ]; then
  echo "setup-proxy.sh: run deploy/setup.sh first" >&2
  exit 1
fi

run install -o root -g root -m 0755 "$HERE/duckdns-update.sh" /usr/local/bin/tazko-duckdns
for u in tazko-duckdns.service tazko-duckdns.timer; do
  run install -o root -g root -m 0644 "$HERE/$u" "/etc/systemd/system/$u"
done
run systemctl daemon-reload
run systemctl enable tazko-duckdns.timer

for port in 80 443; do
  rule=(INPUT -p tcp -m state --state NEW --dport "$port" -j ACCEPT)
  if [ "$DRY_RUN" != 1 ] && iptables -C "${rule[@]}" 2>/dev/null; then
    continue
  fi

  at=1
  if [ "$DRY_RUN" != 1 ]; then
    at=$(iptables -L INPUT --line-numbers -n | awk '$2 == "REJECT" { print $1; exit }')
    at=${at:-1}
  fi
  run iptables -I "${rule[0]}" "$at" "${rule[@]:1}"
done
if [ "$DRY_RUN" != 1 ] && ! command -v netfilter-persistent >/dev/null; then
  run apt-get install -y iptables-persistent
fi
run netfilter-persistent save

if [ "$DRY_RUN" = 1 ] || ! command -v caddy >/dev/null; then
  run apt-get update
  run apt-get install -y caddy
fi
caddyfile=$(mktemp)
trap 'rm -f "$caddyfile"' EXIT
sed "s/TAZKO_HOSTNAME/$HOST/" "$HERE/Caddyfile" >"$caddyfile"
run caddy validate --config "$caddyfile" --adapter caddyfile
if [ -s /etc/caddy/Caddyfile ] && ! cmp -s "$caddyfile" /etc/caddy/Caddyfile; then
  run cp -p /etc/caddy/Caddyfile "/etc/caddy/Caddyfile.before-tazko-$(date -u +%Y%m%d-%H%M%S)"
fi
run install -o root -g root -m 0644 "$caddyfile" /etc/caddy/Caddyfile
run systemctl enable caddy
run systemctl restart caddy

cat <<NEXT

Next:
  1. Add DUCKDNS_DOMAIN=<name> and DUCKDNS_TOKEN=<token> to /etc/tazko/tazko.env (as root; never in a chat or the repository),
     then run: sudo systemctl start tazko-duckdns.service tazko-duckdns.timer && journalctl -u tazko-duckdns -n 5
  2. In /etc/tazko/tazko.env set TAZKO_PUBLIC_URL=https://$HOST and restart: sudo systemctl restart tazko.service
  3. Add https://$HOST/auth/google/callback to the Google client's redirect URIs.
  4. Open https://$HOST (Caddy gets the certificate on the first request; check: journalctl -u caddy -n 20).
NEXT

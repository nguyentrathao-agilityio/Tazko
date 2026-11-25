#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
DEPLOY="$ROOT/deploy"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
pass=0; fail=0

expect() {
  local want=$1 name=$2; shift 2
  if "$@" >"$TMP/out" 2>&1; then got=pass; else got=fail; fi
  if [ "$got" = "$want" ]; then pass=$((pass+1)); echo "  ok   $name"
  else fail=$((fail+1)); echo "  BAD  $name (wanted $want, got $got)"; sed 's/^/       /' "$TMP/out"; fi
}
has() { grep -q -- "$1" "$2"; }

for s in "$DEPLOY"/*.sh "$DEPLOY"/oracle/*.sh; do
  expect pass "bash syntax of $(basename "$s")" bash -n "$s"
done

DRY_RUN=1 bash "$DEPLOY/setup.sh" >"$TMP/setup.txt"
expect pass "setup creates the two accounts" has 'useradd --system --user-group --home-dir /var/lib/tazko --no-create-home --shell /usr/sbin/nologin tazko' "$TMP/setup.txt"
expect pass "setup creates the deploy account" has 'nologin tazko-deploy' "$TMP/setup.txt"
expect pass "settings file is 640, tazko-deploy and the tazko group" has 'install -o tazko-deploy -g tazko -m 0640 .*tazko.env.example /etc/tazko/tazko.env' "$TMP/setup.txt"
expect pass "data folder is private to tazko" has 'install -d -o tazko -g tazko -m 0700 /var/lib/tazko/data' "$TMP/setup.txt"
expect pass "release script is root-owned" has 'install -o root -g root -m 0755 .*release.sh /usr/local/bin/tazko-release' "$TMP/setup.txt"
expect pass "sudoers file is checked" has 'visudo -cf /etc/sudoers.d/tazko-deploy' "$TMP/setup.txt"
expect pass "units and timers are enabled" has 'systemctl enable tazko.service tazko-backup.timer tazko-restore-test.timer' "$TMP/setup.txt"
mkdir "$TMP/bin"; printf '#!/bin/sh\necho 1000\n' >"$TMP/bin/id"; chmod +x "$TMP/bin/id"
expect fail "setup refuses to run without root" env DRY_RUN=0 PATH="$TMP/bin:$PATH" bash "$DEPLOY/setup.sh"
if grep -R -n -E '8790|cloudflared|config\.yml|/etc/cloudflared' "$DEPLOY" >"$TMP/other"; then
  fail=$((fail+1)); echo "  BAD  deploy files touch the other service or the tunnel"; cat "$TMP/other"
else
  pass=$((pass+1)); echo "  ok   deploy files never touch the other service or the tunnel"
fi

unit="$DEPLOY/tazko.service"
for line in 'User=tazko' 'EnvironmentFile=/etc/tazko/tazko.env' 'ExecStart=/opt/tazko/current/tazko serve' 'Restart=on-failure' \
  'NoNewPrivileges=true' 'ProtectSystem=strict' 'ProtectHome=true' 'PrivateTmp=true' 'ReadWritePaths=/var/lib/tazko' \
  'CapabilityBoundingSet=' 'SystemCallArchitectures=native'; do
  expect pass "service has $line" has "^$line" "$unit"
done
expect pass "backup runs at 02:30" has '^OnCalendar=\*-\*-\* 02:30:00' "$DEPLOY/tazko-backup.timer"
expect pass "backup keeps 30 days" has 'backup --data /var/lib/tazko/data --to /var/backups/tazko --keep-days 30' "$DEPLOY/tazko-backup.service"
expect pass "restore test runs weekly" has '^OnCalendar=Sun' "$DEPLOY/tazko-restore-test.timer"
expect pass "restore test records its result" has 'backup verify --from /var/backups/tazko --data /var/lib/tazko/data' "$DEPLOY/tazko-restore-test.service"
expect pass "sudo is limited to restarting the service and reading its log" bash -c "[ \"\$(tr ',' '\n' <'$DEPLOY/sudoers-tazko-deploy' | grep -c '/usr/bin/')\" = 3 ] && ! grep -q 'ALL *$' '$DEPLOY/sudoers-tazko-deploy'"

FAKE="$TMP/bin"; mkdir -p "$FAKE"
cat >"$FAKE/curl" <<'CURL'
#!/usr/bin/env bash
v="$TAZKO_ROOT/current/VERSION"
[ -f "$v" ] && ! grep -q broken "$v"
CURL
chmod +x "$FAKE/curl"

export TAZKO_ROOT="$TMP/opt"
export TAZKO_HEALTH_WAIT=1
export TAZKO_RESTART="true"
export PATH="$FAKE:$PATH"
mkdir -p "$TAZKO_ROOT/releases"

bundle() {
  local name=$1 version=$2 extra=${3:-}
  local d="$TMP/b-$name"; mkdir -p "$d"
  printf '#!/bin/sh\necho tazko %s\n' "$version" >"$d/tazko"; chmod +x "$d/tazko"
  echo "$version" >"$d/VERSION"
  if [ -n "$extra" ]; then echo x >"$d/$extra"; tar -czf "$TMP/tazko-$name.tar.gz" -C "$d" tazko VERSION "$extra"
  else tar -czf "$TMP/tazko-$name.tar.gz" -C "$d" tazko VERSION; fi
  echo "$TMP/tazko-$name.tar.gz"
}
current() { basename "$(readlink "$TAZKO_ROOT/current")"; }
rel() { bash "$DEPLOY/release.sh" "$@"; }

expect pass "first release goes live" rel "$(bundle 20261118-0100-aaaaaaa 1.0)"
expect pass "current points at it" test "$(current)" = 20261118-0100-aaaaaaa
expect pass "second release goes live" rel "$(bundle 20261118-0200-bbbbbbb 1.1)"
expect fail "a broken release rolls itself back" rel "$(bundle 20261118-0300-ccccccc broken)"
cp "$TMP/out" "$TMP/rollback.txt"
expect pass "current is the previous release again" test "$(current)" = 20261118-0200-bbbbbbb
expect pass "the rollback is reported" has 'rolled back to 20261118-0200-bbbbbbb' "$TMP/rollback.txt"
expect pass "list marks the current release" bash -c "bash '$DEPLOY/release.sh' --list | grep -q '^\* 20261118-0200-bbbbbbb'"
expect pass "manual rollback goes one release back" rel --rollback
expect pass "current is the first release" test "$(current)" = 20261118-0100-aaaaaaa
expect fail "no older release to roll back to" rel --rollback
expect fail "the same bundle cannot be released twice" rel "$TMP/tazko-20261118-0100-aaaaaaa.tar.gz"
expect fail "a bundle with extra files is refused" rel "$(bundle 20261118-0400-ddddddd 1.2 notes.txt)"
expect fail "a bundle with a bad name is refused" bash -c "cp '$TMP/tazko-20261118-0200-bbbbbbb.tar.gz' '$TMP/other.tar.gz' && bash '$DEPLOY/release.sh' '$TMP/other.tar.gz'"
expect fail "a binary that does not run is refused" bash -c "d='$TMP/b-bad'; mkdir -p \$d; printf 'not a program' >\$d/tazko; echo 9 >\$d/VERSION; tar -czf '$TMP/tazko-20261118-0450-eeeeeee.tar.gz' -C \$d tazko VERSION; bash '$DEPLOY/release.sh' '$TMP/tazko-20261118-0450-eeeeeee.tar.gz'"
expect pass "a refused bundle leaves the live release alone" test "$(current)" = 20261118-0100-aaaaaaa
for i in 5 6 7 8 9; do rel "$(bundle "20261118-0${i}00-f${i}f${i}f${i}f" "2.$i")" >/dev/null; done
expect pass "only the last five releases are kept" test "$(find "$TAZKO_ROOT/releases" -mindepth 1 -maxdepth 1 -type d | wc -l | tr -d ' ')" = 5
expect pass "the newest is live" test "$(current)" = 20261118-0900-f9f9f9f

BINARY="$TMP/b-20261118-0100-aaaaaaa/tazko" OUT="$TMP/dist" bash "$DEPLOY/bundle.sh" >"$TMP/bundle-path"
expect pass "bundle.sh makes a bundle the release script accepts" bash -c "tar -tzf \"\$(cat '$TMP/bundle-path')\" | sort | tr '\n' ' ' | grep -qx 'VERSION tazko '"
expect pass "bundle name has time and commit" bash -c "basename \"\$(cat '$TMP/bundle-path')\" | grep -Eq '^tazko-[0-9]{8}-[0-9]{6}-[0-9a-f]{12}\.tar\.gz$'"

SRC="$TMP/src"; mkdir -p "$SRC/deploy"
cp "$DEPLOY/bundle.sh" "$SRC/deploy/"; cp "$ROOT/Cargo.toml" "$SRC/"
expect pass "bundle.sh outside a repository takes TAZKO_COMMIT" bash -c "cd '$SRC' && GIT_CEILING_DIRECTORIES='$TMP' TAZKO_COMMIT=0123456789ab BINARY='$TMP/b-20261118-0100-aaaaaaa/tazko' OUT='$TMP/dist2' bash deploy/bundle.sh | grep -Eq 'tazko-[0-9]{8}-[0-9]{6}-0123456789ab\.tar\.gz$'"
expect fail "bundle.sh refuses a bad TAZKO_COMMIT" bash -c "cd '$SRC' && GIT_CEILING_DIRECTORIES='$TMP' TAZKO_COMMIT='x; rm' BINARY='$TMP/b-20261118-0100-aaaaaaa/tazko' OUT='$TMP/dist2' bash deploy/bundle.sh"
expect fail "send-source.sh needs a destination" bash "$DEPLOY/oracle/send-source.sh"

ORACLE="$DEPLOY/oracle"
DRY_RUN=1 bash "$ORACLE/setup-proxy.sh" tazko-demo.duckdns.org >"$TMP/proxy.txt"
expect pass "proxy setup installs caddy from the system packages" has 'apt-get install -y caddy' "$TMP/proxy.txt"
expect pass "proxy setup opens 80" has '--dport 80 -j ACCEPT' "$TMP/proxy.txt"
expect pass "proxy setup opens 443" has '--dport 443 -j ACCEPT' "$TMP/proxy.txt"
expect pass "firewall rules are saved" has 'netfilter-persistent save' "$TMP/proxy.txt"
expect pass "the Caddyfile is checked before it is installed" bash -c "grep -n 'caddy validate --config /' '$TMP/proxy.txt' | grep -v /etc/caddy/Caddyfile | grep -q . && [ \$(grep -n 'caddy validate' '$TMP/proxy.txt' | cut -d: -f1) -lt \$(grep -n 'Caddyfile$' '$TMP/proxy.txt' | tail -1 | cut -d: -f1) ]"
expect pass "the DuckDNS timer is enabled" has 'systemctl enable tazko-duckdns.timer' "$TMP/proxy.txt"
expect pass "the next steps start the DuckDNS timer" has 'systemctl start tazko-duckdns.service tazko-duckdns.timer' "$TMP/proxy.txt"
expect pass "the Google redirect is named" has 'https://tazko-demo.duckdns.org/auth/google/callback' "$TMP/proxy.txt"
expect pass "caddy proxies to the local server only" has 'reverse_proxy 127.0.0.1:8791' "$ORACLE/Caddyfile"
expect pass "caddy replaces the client address header with the real one" has 'header_up Cf-Connecting-Ip {remote_host}' "$ORACLE/Caddyfile"
for bad in "" "Tazko.duckdns.org" "a;b.duckdns.org" "localhost" "x.duckdns.org/evil"; do
  expect fail "proxy setup refuses the host name '$bad'" env DRY_RUN=1 bash "$ORACLE/setup-proxy.sh" "$bad"
done
expect pass "the DuckDNS unit runs as a throwaway user" has '^DynamicUser=true' "$ORACLE/tazko-duckdns.service"
expect pass "the DuckDNS unit gets the token as a credential" has '^LoadCredential=tazko.env:/etc/tazko/tazko.env' "$ORACLE/tazko-duckdns.service"

DNS="$TMP/dns"; mkdir -p "$DNS/bin"
cat >"$DNS/bin/curl" <<'CURL'
#!/usr/bin/env bash
printf '%s\n' "$*" >"$DUCKDNS_LOG.args"
cat >"$DUCKDNS_LOG.stdin"
cat "$DUCKDNS_ANSWER"
CURL
chmod +x "$DNS/bin/curl"
token=0a1b2c3d-1111-2222-3333-444455556666
printf 'TAZKO_PUBLIC_URL=https://tazko-demo.duckdns.org\nDUCKDNS_DOMAIN=Tazko-Demo\nDUCKDNS_TOKEN=%s\n' "$token" >"$DNS/duckdns.env"
printf 'OK' >"$DNS/ok"; printf 'KO' >"$DNS/ko"
duck() { env PATH="$DNS/bin:$PATH" TAZKO_DUCKDNS_ENV="$1" DUCKDNS_LOG="$DNS/log" DUCKDNS_ANSWER="$2" bash "$ORACLE/duckdns-update.sh"; }
expect pass "DuckDNS update succeeds on OK" duck "$DNS/duckdns.env" "$DNS/ok"
expect pass "the token never appears in the command line" bash -c "! grep -q '$token' '$DNS/log.args' && grep -q 'token=$token' '$DNS/log.stdin'"
expect pass "only https is allowed" has "--proto =https" "$DNS/log.args"
expect fail "DuckDNS update fails on KO" duck "$DNS/duckdns.env" "$DNS/ko"
printf 'DUCKDNS_DOMAIN=tazko-demo\nDUCKDNS_TOKEN=not-a-token\n' >"$DNS/bad.env"
expect fail "a malformed token is refused before any request" duck "$DNS/bad.env" "$DNS/ok"
printf 'DUCKDNS_DOMAIN=a&b\nDUCKDNS_TOKEN=%s\n' "$token" >"$DNS/bad2.env"
expect fail "a malformed domain is refused" duck "$DNS/bad2.env" "$DNS/ok"
printf 'DUCKDNS_TOKEN=%s\n' "$token" >"$DNS/missing.env"
duck "$DNS/missing.env" "$DNS/ok" >"$TMP/duck-missing.txt" 2>&1 || true
expect pass "a missing domain is named in the error" has 'DUCKDNS_DOMAIN' "$TMP/duck-missing.txt"
expect pass "the domain is sent in lower case" has 'domains=tazko-demo&' "$DNS/log.stdin"
duck "$DNS/duckdns.env" "$DNS/ko" >"$TMP/duck-ko.txt" 2>&1 || true
expect fail "a refused update does not print the token" has "$token" "$TMP/duck-ko.txt"

echo "  $pass passed, $fail failed"
[ "$fail" -eq 0 ]

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

for s in "$DEPLOY"/*.sh; do
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

echo "  $pass passed, $fail failed"
[ "$fail" -eq 0 ]

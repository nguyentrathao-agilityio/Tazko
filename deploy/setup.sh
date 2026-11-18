#!/usr/bin/env bash
set -euo pipefail

DRY_RUN=${DRY_RUN:-0}
HERE=$(cd "$(dirname "$0")" && pwd)
UNITS="tazko.service tazko-backup.service tazko-backup.timer tazko-restore-test.service tazko-restore-test.timer"

run() {
  printf '+ %s\n' "$*"
  if [ "$DRY_RUN" != 1 ]; then
    "$@"
  fi
}

if [ "$DRY_RUN" != 1 ] && [ "$(id -u)" -ne 0 ]; then
  echo "setup.sh: run as root (sudo), or with DRY_RUN=1 to print the commands" >&2
  exit 2
fi

if [ "$DRY_RUN" != 1 ] && command -v ss >/dev/null && ss -ltnH 'sport = :8791' | grep -q .; then
  echo "setup.sh: something already listens on port 8791; stop and check before going on" >&2
  exit 1
fi

for u in tazko tazko-deploy; do
  if ! id "$u" >/dev/null 2>&1; then
    home=/var/lib/tazko
    [ "$u" = tazko-deploy ] && home=/opt/tazko
    run useradd --system --user-group --home-dir "$home" --no-create-home --shell /usr/sbin/nologin "$u"
  fi
done

run install -d -o tazko -g tazko -m 0750 /var/lib/tazko
run install -d -o tazko -g tazko -m 0700 /var/lib/tazko/data
run install -d -o tazko -g tazko -m 0700 /var/backups/tazko
run install -d -o tazko-deploy -g tazko-deploy -m 0755 /opt/tazko
run install -d -o tazko-deploy -g tazko-deploy -m 0755 /opt/tazko/releases
run install -d -o tazko-deploy -g tazko-deploy -m 0750 /var/lib/tazko-incoming
run install -d -o tazko-deploy -g tazko -m 0750 /etc/tazko

if [ "$DRY_RUN" = 1 ] || [ ! -e /etc/tazko/tazko.env ]; then
  run install -o tazko-deploy -g tazko -m 0640 "$HERE/tazko.env.example" /etc/tazko/tazko.env
else
  run chown tazko-deploy:tazko /etc/tazko/tazko.env
  run chmod 0640 /etc/tazko/tazko.env
fi

run install -o root -g root -m 0755 "$HERE/release.sh" /usr/local/bin/tazko-release
run install -o root -g root -m 0440 "$HERE/sudoers-tazko-deploy" /etc/sudoers.d/tazko-deploy
run visudo -cf /etc/sudoers.d/tazko-deploy

for u in $UNITS; do
  run install -o root -g root -m 0644 "$HERE/$u" "/etc/systemd/system/$u"
done
run systemctl daemon-reload
run systemctl enable tazko.service tazko-backup.timer tazko-restore-test.timer

cat <<'NEXT'

Next:
  1. Put the Google client id and secret into /etc/tazko/tazko.env (as root; never in a chat or the repository).
  2. Copy a bundle into /var/lib/tazko-incoming and run, as tazko-deploy:
       sudo -u tazko-deploy tazko-release /var/lib/tazko-incoming/tazko-<time>-<commit>.tar.gz
  3. Create the first super admin:
       sudo -u tazko /opt/tazko/current/tazko admin --data /var/lib/tazko/data --email <address>
  4. Start the timers: sudo systemctl start tazko-backup.timer tazko-restore-test.timer
NEXT

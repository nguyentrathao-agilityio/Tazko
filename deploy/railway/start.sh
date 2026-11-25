#!/bin/sh
set -eu
umask 077

volume="${TAZKO_VOLUME:-/var/lib/tazko}"
backups="$volume/backups"
keep_days="${TAZKO_BACKUP_KEEP_DAYS:-14}"
export TAZKO_DATA_DIR="$volume/data"
export TAZKO_PORT="${TAZKO_PORT:-${PORT:-8791}}"
export TAZKO_CLIENT_IP_HEADER="${TAZKO_CLIENT_IP_HEADER:-x-forwarded-for}"

if ! mountpoint -q "$volume"; then
    echo "tazko-start: no volume is mounted at $volume; add one there so the data survives a deploy" >&2
    exit 1
fi

mkdir -p "$TAZKO_DATA_DIR" "$backups"
chown -R tazko:tazko "$volume"
chmod 700 "$volume" "$TAZKO_DATA_DIR" "$backups"

as_tazko() {
    setpriv --reuid=tazko --regid=tazko --init-groups --no-new-privs "$@"
}

older_than() {
    [ -z "$(find "$1" -maxdepth 0 -mmin "-$2" 2>/dev/null)" ]
}

(
    sleep 600
    while :; do
        if older_than "$backups/.last-backup" 1380; then
            as_tazko tazko backup --data "$TAZKO_DATA_DIR" --to "$backups" --keep-days "$keep_days" &&
                as_tazko touch "$backups/.last-backup" ||
                echo "tazko-start: backup failed" >&2
        fi
        if older_than "$backups/.last-verify" 10020; then
            as_tazko tazko backup verify --from "$backups" --data "$TAZKO_DATA_DIR" --scratch /tmp &&
                as_tazko touch "$backups/.last-verify" ||
                echo "tazko-start: restore test failed" >&2
        fi
        sleep 3600
    done
) &

exec setpriv --reuid=tazko --regid=tazko --init-groups --no-new-privs tazko serve

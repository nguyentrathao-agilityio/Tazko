# Deployment

How Tazko is installed, released, rolled back and backed up on an Ubuntu machine. Step 1.11 tries this on the local test VM. There is no public hostname yet: the server listens on `127.0.0.1:8791` only and is reached through an SSH port forward.

**Every command below that changes a machine is shown to the owner and waits for approval.** Nothing here touches another service, its accounts or folders, or a tunnel.

## What gets installed

| Path | Owner | Mode | What |
| --- | --- | --- | --- |
| `/opt/tazko/releases/<time>-<commit>/` | tazko-deploy | 755 | One folder per release: `tazko` and `VERSION`. The last 5 are kept. |
| `/opt/tazko/current` | tazko-deploy | link | Points at the live release. |
| `/etc/tazko/tazko.env` | tazko-deploy:tazko | 640 | Settings, including the Google client secret. |
| `/var/lib/tazko/data/` | tazko | 700 | `control.db`, `orgs/`, `trash/`, `secret`, `status/`. |
| `/var/backups/tazko/` | tazko | 700 | Nightly snapshots, kept 30 days. |
| `/var/lib/tazko-incoming/` | tazko-deploy | 750 | Bundles waiting to be released. |
| `/usr/local/bin/tazko-release` | root | 755 | The release script. |
| `/etc/sudoers.d/tazko-deploy` | root | 440 | Lets `tazko-deploy` restart `tazko.service` and read its log, nothing else. |
| `/etc/systemd/system/tazko*.{service,timer}` | root | 644 | The service, the nightly backup (02:30) and the weekly restore test (Sunday 04:00). |

Accounts: `tazko` runs the service and can write only `/var/lib/tazko` (and the backups); `tazko-deploy` owns the releases and settings. Neither can log in.

## Build a bundle

On a Linux build machine with the musl target (`rustup target add x86_64-unknown-linux-musl`, and `musl-tools` on Ubuntu):

```bash
deploy/bundle.sh
```

It prints `dist/tazko-<YYYYMMDD-HHMMSS>-<commit>.tar.gz`, which holds exactly `tazko` and `VERSION`.

## First install

Copy the repository's `deploy/` folder to the machine, then review what the setup would do:

```bash
DRY_RUN=1 bash deploy/setup.sh
```

After approval, run it for real:

```bash
sudo bash deploy/setup.sh
```

It refuses to run if something already listens on port 8791. Then:

1. Fill `TAZKO_GOOGLE_CLIENT_ID`, `TAZKO_GOOGLE_CLIENT_SECRET` and `TAZKO_PUBLIC_URL` in `/etc/tazko/tazko.env` as root. With an SSH port forward, keep `TAZKO_PUBLIC_URL=http://localhost:8791` and use the "Tazko local" Google client. Optional: the `TAZKO_SMTP_*` and `TAZKO_MAIL_FROM` settings for email (`docs/email.md`).
2. Release the first bundle (below).
3. Create the first super admin: `sudo -u tazko /opt/tazko/current/tazko admin --data /var/lib/tazko/data --email <address>`.
4. Start the timers: `sudo systemctl start tazko-backup.timer tazko-restore-test.timer`.

From a laptop: `ssh -L 8791:127.0.0.1:8791 <vm>` and open `http://localhost:8791`.

## Release and roll back

```bash
sudo -u tazko-deploy tazko-release /var/lib/tazko-incoming/tazko-<time>-<commit>.tar.gz
sudo -u tazko-deploy tazko-release --list
sudo -u tazko-deploy tazko-release --rollback
```

A release refuses a bundle that is not named `tazko-<time>-<commit>.tar.gz`, holds anything but `tazko` and `VERSION`, was already released, or whose binary does not run. It then points `current` at the new release, restarts the service and polls `http://127.0.0.1:8791/health` for 20 seconds. If the new release is not healthy, it points `current` back, restarts and exits with an error. After a healthy release it keeps the 5 newest releases.

## Backups and restore

- Nightly at 02:30: `tazko backup --data /var/lib/tazko/data --to /var/backups/tazko --keep-days 30` copies every database online (safe while the service runs) and every stored file into a dated snapshot, checks the copy and records the result. The newest snapshot is never removed.
- Sundays at 04:00: `tazko backup verify` restores the newest snapshot into a scratch folder, runs the check and records the result. Both results show on the admin Server page.
- `tazko check --data <folder>` runs `PRAGMA integrity_check` on every database and compares every stored file with its content hash.

The server secret is not in the backups; a restored server makes a new one, which only signs people out of half-finished sign-ins.

To restore:

```bash
sudo systemctl stop tazko.service
sudo -u tazko /opt/tazko/current/tazko backup restore --from /var/backups/tazko --to /var/lib/tazko/data.new
sudo -u tazko /opt/tazko/current/tazko check --data /var/lib/tazko/data.new
sudo mv /var/lib/tazko/data /var/lib/tazko/data.old
sudo mv /var/lib/tazko/data.new /var/lib/tazko/data
sudo systemctl start tazko.service
```

`backup restore` only writes into a new, empty folder and checks the result. Add `--snapshot <YYYYMMDD-HHMMSS>` for an older snapshot.

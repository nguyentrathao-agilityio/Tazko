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

## Railway

A managed alternative to the Ubuntu machine: Railway builds the `Dockerfile` from the repository and runs one container with a volume for the data. It gives a fixed `https://<name>.up.railway.app` address, so Google sign-in keeps one redirect. Everything in this section is done by the owner in the Railway and Google consoles; no secret goes into the repository or a chat.

### What the image does

- `Dockerfile` builds the web app (Node 24, pnpm), then the server (`cargo build --release --locked` with the web app embedded), and copies only the binary and `deploy/railway/start.sh` into a small Debian image with the system certificates.
- `deploy/railway/start.sh` refuses to start unless a volume is mounted at `/var/lib/tazko`, so data never lands on the container's throwaway disk. It keeps the data in `/var/lib/tazko/data`, makes the volume owned by an unprivileged `tazko` user and owner-only (umask 077), and runs `tazko serve` as that user without new privileges, listening on all addresses (IPv4 and IPv6) at Railway's `PORT` unless `TAZKO_PORT` is set.
- Backups: ten minutes after start, then every hour, it takes a `tazko backup` snapshot into `/var/lib/tazko/backups` when the last one is more than 23 hours old (kept 14 days, `TAZKO_BACKUP_KEEP_DAYS`), and runs the restore test `tazko backup verify` when the last one is more than a week old. Both results show on Admin, Server. The times are kept in files on the volume, so frequent deploys do not postpone them.
- Sign-in start limits count each visitor by the last address in `X-Forwarded-For`, which Railway's proxy adds (`TAZKO_CLIENT_IP_HEADER`, default `cf-connecting-ip` elsewhere for the tunnel).
- `railway.json` builds from the `Dockerfile`, runs one replica (SQLite files allow one writer), checks `/health` before switching traffic, and restarts on failure.

### First deploy

1. In Railway, **New project → Deploy from GitHub repo**, pick the Tazko repository and the `dev` branch.
2. On the service, **Settings → Volumes → Add volume**, mount path `/var/lib/tazko`.
3. **Settings → Networking → Generate domain**. Note the address, for example `https://tazko-production.up.railway.app`.
4. **Variables** (names only here; type the values in Railway):
   - `TAZKO_PUBLIC_URL` = the address from step 3, with `https://` and no trailing slash
   - `TAZKO_GOOGLE_CLIENT_ID`, `TAZKO_GOOGLE_CLIENT_SECRET`
   - `TAZKO_SECRET`: leave unset to use the `secret` file the server creates on the volume, or 32+ random characters
   - optional email: `TAZKO_SMTP_HOST`, `TAZKO_SMTP_PORT`, `TAZKO_SMTP_USER`, `TAZKO_SMTP_PASSWORD`, `TAZKO_MAIL_FROM` (see `docs/email.md`)
   - never set `TAZKO_WEBHOOKS_ALLOW_LOCAL` here
5. In the Google Cloud console (project Tazko), add the redirect URI `https://<address>/auth/google/callback` to the web OAuth client, and either publish the app or list the people who sign in as test users.
6. Deploy. The deployment is healthy when `https://<address>/health` answers. If the log says no volume is mounted, add the volume from step 2 with exactly that path.
7. Create the first super admin from the Railway CLI (`railway link`, then `railway ssh`), as the `tazko` user so the files keep their owner:

   ```bash
   setpriv --reuid=tazko --regid=tazko --init-groups tazko admin --data /var/lib/tazko/data --email <your Google address>
   ```

8. Sign in with that Google account, then send a test email from Admin, Server.

### Updates and rollback

Every push to the chosen branch builds and deploys a new image. Because the volume attaches to one container at a time, Railway stops the old container before the new one starts, so each deploy has a short outage (the new container runs any migrations and must answer `/health`); open boards reconnect by themselves. Deploy outside working hours when possible. To go back, open **Deployments**, pick an earlier one and choose **Redeploy**. The data on the volume stays; a newer release may already have migrated it, so roll back only to the release just before.

### Backups on Railway

Snapshots are written to the same volume, so they protect against mistakes and bad releases, not against losing the volume. To keep a copy elsewhere, open `railway ssh` and list `/var/lib/tazko/backups`, then copy a snapshot out with the Railway CLI, or use the volume backups Railway offers on the plan in use. `tazko check` runs in the container at any time. A restore needs the server stopped, which `railway ssh` cannot do from inside the running container: set the service's custom start command to `sleep infinity` for the restore, run the restore steps above as the `tazko` user with `/var/lib/tazko` paths, then clear the start command and redeploy.

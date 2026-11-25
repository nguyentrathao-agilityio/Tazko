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

## Oracle Always Free + DuckDNS

A free, always-on public server: an Oracle Cloud Always Free ARM VM, a free `<name>.duckdns.org` name, and Caddy in front of Tazko for HTTPS with an automatic Let's Encrypt certificate. Tazko itself still listens on `127.0.0.1:8791` only; Caddy is the only thing reachable from outside. Check Oracle's current Always Free limits before you start.

### 1. The VM (owner, in the Oracle console)

1. Create an instance: image **Ubuntu 24.04**, shape **VM.Standard.A1.Flex** (ARM; 1 to 2 OCPU and 6 to 12 GB are plenty), **Assign a public IPv4 address**, and paste your SSH **public** key.
2. In the instance's subnet, open **Security Lists → Default security list → Add ingress rules**: source `0.0.0.0/0`, TCP, destination port `80`; and the same for `443`. Leave SSH (22) as it is. Port 8791 is never opened.
3. Note the public IP and log in: `ssh ubuntu@<public IP>`.

Oracle's Ubuntu images also reject everything but SSH in their own `iptables` rules; step 4 opens 80 and 443 there too.

### 2. The DuckDNS name (owner, on duckdns.org)

Sign in, add a sub domain (for example `tazko-demo`), point it at the VM's public IP, and keep the account's token private. The token goes only into `/etc/tazko/tazko.env` on the VM (step 4), with the other secrets, never into the repository or a chat.

### 3. Build and install Tazko on the VM

The VM is ARM, so the binary is built on the VM itself; nothing else in the release, rollback or backup scripts changes. On the laptop, from a clean checkout of the commit to deploy:

```bash
deploy/oracle/send-source.sh ubuntu@<public IP>
```

It refuses to run with uncommitted or untracked files, builds the web app from the committed files only, packs them with `web/dist` and a `COMMIT` file, and copies `tazko-src-<commit>.tar.gz` to the VM. On the VM, the first time only:

```bash
sudo apt-get update && sudo apt-get install -y build-essential curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
```

Then for every release (on the VM, in the home folder):

```bash
rm -rf tazko && tar -xzf tazko-src-<commit>.tar.gz && cd tazko
TAZKO_SKIP_WEB_BUILD=1 TARGET=aarch64-unknown-linux-gnu TAZKO_COMMIT=$(cat COMMIT) CARGO_TARGET_DIR=~/tazko-target deploy/bundle.sh
```

The build folder `~/tazko-target` stays between releases, so only Tazko itself is compiled again; the first build takes a while on a small VM. The build competes with the running server for CPU for those minutes. The first time, run `sudo bash deploy/setup.sh` from that folder (see "First install" above). Then release as usual:

```bash
sudo install -o tazko-deploy -g tazko-deploy -m 0640 dist/tazko-<time>-<commit>.tar.gz /var/lib/tazko-incoming/
sudo -u tazko-deploy tazko-release /var/lib/tazko-incoming/tazko-<time>-<commit>.tar.gz
```

### 4. HTTPS with Caddy and the DuckDNS updater

From the same folder on the VM, review and then run:

```bash
DRY_RUN=1 bash deploy/oracle/setup-proxy.sh <name>.duckdns.org
sudo bash deploy/oracle/setup-proxy.sh <name>.duckdns.org
```

It installs:

| What | Where | Notes |
| --- | --- | --- |
| DuckDNS updater | `/usr/local/bin/tazko-duckdns`, `tazko-duckdns.service`, `tazko-duckdns.timer` | Every 5 minutes and at boot. Runs as a throwaway system user that gets `/etc/tazko/tazko.env` as a systemd credential and reads only `DUCKDNS_DOMAIN` (the part before `.duckdns.org`) and `DUCKDNS_TOKEN` from it. The token is sent to `https://www.duckdns.org` only, passed to `curl` on standard input so it never shows in the process list or logs. |
| Firewall | `iptables` rules for TCP 80 and 443, saved with `netfilter-persistent` | Added once, before Oracle's REJECT rule; `iptables-persistent` is installed if it is missing, so the rules survive a reboot. |
| Caddy | Ubuntu's `caddy` package, `/etc/caddy/Caddyfile` | Proxies `<name>.duckdns.org` to `127.0.0.1:8791` and sets the client address header Tazko reads to the real visitor address, replacing anything the visitor sent. Caddy redirects HTTP to HTTPS and gets and renews the certificate itself. The new file is checked before it is installed; an existing different Caddyfile is kept as `Caddyfile.before-tazko-<time>`. |

Then, as the script prints:

1. Add `DUCKDNS_DOMAIN=<name>` and `DUCKDNS_TOKEN=<token>` to `/etc/tazko/tazko.env` with `sudo nano`, run `sudo systemctl start tazko-duckdns.service tazko-duckdns.timer` and check `journalctl -u tazko-duckdns -n 5` says the name points at this machine.
2. In `/etc/tazko/tazko.env` set `TAZKO_PUBLIC_URL=https://<name>.duckdns.org` and the Google client id and secret, then `sudo systemctl restart tazko.service`.
3. In Google Cloud Console → the OAuth client, add the redirect URI `https://<name>.duckdns.org/auth/google/callback`. While the consent screen is in Testing, add each person who signs in under Test users.
4. Open `https://<name>.duckdns.org`. The first request takes a few seconds while Caddy gets the certificate (`journalctl -u caddy -n 20` shows it).
5. Create the first super admin and start the backup timers (see "First install").

### Email from the VM

Oracle blocks outgoing port 25 but usually not 587. Test it from the VM before relying on it:

```bash
timeout 5 bash -c '</dev/tcp/smtp.gmail.com/587' && echo "587 is open"
```

If it is open, put the `TAZKO_SMTP_*` and `TAZKO_MAIL_FROM` settings into `/etc/tazko/tazko.env` (`docs/email.md`), restart, and press **Send test email** on the admin Server page.

### Known limits

- An always-free VM has no support and Oracle may reclaim idle Always Free instances; keep the nightly backups, and copy one off the VM now and then (`scp ubuntu@<ip>:/var/backups/tazko/...`).

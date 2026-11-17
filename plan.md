# Tazko build plan v2

Build plan v2 · single binary behind a Cloudflare Tunnel · for handoff to a Linux build session

> This is the full build plan (v2, 16 Nov 2026) with the decisions taken in step 0.1. Where the plan mentions `docs/build-plan.html`, this file is that plan. The visual reference is `design/tazko-prototype.html`.

A self-hosted kanban work tracker for several companies. One Rust binary, one SQLite file per company, a super admin console to run the companies, and Google sign-in for both Workspace and personal accounts.

One static Rust binary · Web app and admin console embedded · SQLite per company · Google: Workspace + Gmail · Cloudflare Tunnel, no open port

v2 · 16 Nov 2026 · Visual reference: design/tazko-prototype.html · Replaces the first build plan.

## What changed in v2

v2 applies the review of 13 and 16 Nov: the prototype was compared with the board tool the team uses today (10 boards, 581 cards) and with that tool's documented behaviour. Every change below is marked (v2) where it appears in the plan. Decisions taken on the owner's behalf are listed under [Assumptions and open items](#assumptions-and-open-items) for confirmation.

| Area       | Change                                                                                                        | Why                                                                                                      |
| ---------- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| Data       | **Import** a board from an export file and **export** a board or company, both in M2.                         | Without import the team cannot move its boards; export proves data ownership, the first reason to build. |
| Cards      | **Save any card as a template**, next to the built-in ones.                                                   | The first plan said "saved template" but had no way to save one.                                         |
| My cards   | Current company by default, with an **"All companies"** switch.                                               | v1 and the prototype disagreed on scope.                                                                 |
| Operations | A weekly **restore test** of the nightly backup, a **local development target** and a **security checklist**. | v1 never tested that a backup restores, and had no safe place to practise.                               |
| Plan text  | Milestones now match the prototype v2 screen index; prompts agree that machine changes always need approval.  | v1 rounds and milestones differed; two prompts contradicted each other.                                  |

## The plan as a board

Milestones are lists. Each one ships something usable. The dashed column is what we decided not to build.

#### M1 Platform (8)

- Binary with embedded web app
- Tunnel, systemd, deploy with rollback
- Google sign-in, Workspace and Gmail
- Super admin console
- Create, suspend, delete companies
- Invite owners and members
- Nightly backup and restore test (v2)
- Local development target (v2)

#### M2 Projects and boards (13)

- Boards, lists, cards, drag and drop
- Card detail, checklists, comments
- Labels, assignees, due dates
- Manage projects (company owners)
- Project settings, owners, members, observers
- Board access inside a project
- Card and list actions, shortcuts
- Import and export (v2)
- Personal settings
- Rich-text description
- Search, filter, activity log
- Zen mode, key help, key map
- Card menu, side comments, long cards

#### M3 Live and files (6)

- Real-time board sync
- File attachments (10 MB)
- In-app notifications, @mentions
- Custom fields
- Reactions on comments
- Uploaded board backgrounds

#### M4 Views and templates (5)

- Table view
- My cards (with all companies) (v2)
- Board templates; save any card as template (v2)
- Dashboard view
- Command palette

#### M5 Card depth (4)

- Checklist item assignee and due date
- Recurring cards
- Links between cards
- Voting

#### M6 Integration and email (6)

- Webhooks
- Command line tool
- Email invites and notifications
- Reminders by email
- REST API and tokens
- Notification preferences

#### Later (4)

- Desktop app (Tauri)
- Vietnamese interface
- Production host
- Card on several boards (mirror)

#### Not building (5)

- Rule-based automation
- Third-party add-ons
- Personal planner and inbox
- Timeline, calendar
- Map view

## Decisions

| Topic             | Decision                                                                                                                                                                                                                                                                                                                          | Source                        |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- |
| Why build         | Data ownership and custom workflows. When feature parity with common kanban tools and a company's workflow conflict, the workflow wins.                                                                                                                                                                                           | Owner                         |
| Deployment        | One static Rust binary (musl) with the web app compiled in. Runs under systemd on Ubuntu. Cloudflare Tunnel to `127.0.0.1:8791`. Hostname `tazko-dev.hugepigstudio.com`.                                                                                                                                                          | Owner                         |
| Machine           | An existing Ubuntu server (192.168.86.49) that already runs another service and tunnel. Tazko must coexist with it. Practice builds run on localhost instead. (v2)                                                                                                                                                                | Owner; local target assumed   |
| Database          | SQLite in WAL mode. `control.db` for people, companies, invites, sessions, audit log. One `tazko.db` per company for everything else.                                                                                                                                                                                             | Owner                         |
| Sign-in           | Google only, Workspace and personal accounts. Invite-only: an account can sign in only if its verified email already belongs to a person or a pending invite. After the first sign-in people are identified by Google's `sub`, so an email change does not lock them out.                                                         | Owner                         |
| Super admin       | A flag on a person, set from the command line. First one: `trungngo@hugepigstudio.com`. Manages companies and people from `/admin`. Sees project names, owners and counts but not boards. Every admin action is audit-logged.                                                                                                     | Owner                         |
| Company roles     | Owner and member. Owners manage the company's members, other owners, invites and storage, and manage all projects. Only owners create projects.                                                                                                                                                                                   | Owner                         |
| Projects          | Company, then project, then board. Only company owners create projects, choose their owners, and can archive or delete any project. Deleted projects stay in trash for 30 days. Company owners open a project's boards only after adding themselves, which is audit-logged.                                                       | Owner                         |
| Project access    | Roles: owner, member, observer (read only). Members see every board unless it is restricted. Switches: members can create boards (on), members can invite (off).                                                                                                                                                                  | Owner                         |
| Outside people    | No guest role. Someone invited only to a project joins the company as a plain member who sees only that project.                                                                                                                                                                                                                  | Assumed                       |
| Backend           | Rust: Axum, rusqlite (bundled SQLite), tokio.                                                                                                                                                                                                                                                                                     | Owner                         |
| Web app           | React, TypeScript, Vite, Tailwind, shadcn/ui, dnd-kit. Built to static files, brotli pre-compressed, embedded in the binary. English with strings in an i18n file.                                                                                                                                                                | Assumed                       |
| Desktop           | Deferred.                                                                                                                                                                                                                                                                                                                         | Owner                         |
| Import and export | Import a board from a board export file (JSON): lists, cards, labels, checklists, dates, members matched by email then name, comments, archived cards, link attachments. Export a board or a whole company as JSON. Owners only for company export. (v2)                                                                          | Assumed                       |
| Command line      | One program, `tazko`, is both the server (`serve`, `admin`, `backup`, `check`) and a command-line client (`auth`, `project`, `board`, `card`, `my`, `search`, `token`, `webhook`, `api`). Signs in through the browser with a device code. Builds for Linux, macOS and Windows are served at `/cli`.                              | Owner, shape assumed          |
| Personal settings | Profile, appearance (theme, zoom 80–150% scaling the whole interface including dialogs, density, reduce motion, zen options), date and time, notifications, tokens, sessions. Stored per person, the same in every company.                                                                                                       | Owner                         |
| Email             | Optional, set up in the server settings file. Invitation, mention, assignment and due-date emails. When off, everything works in the app and invitations are shared by hand.                                                                                                                                                      | Owner                         |
| Integrations      | Personal tokens act as the person and never exceed their access. Company API tokens (chosen projects, expiry) and webhooks (HMAC signed, retries for a day, delivery log) arrive with the REST API in M6.                                                                                                                         | Owner, details assumed        |
| Card depth        | Checklist items with assignee and due date; recurring cards; links (relates to, blocks, is blocked by, duplicates) with a Blocked badge; emoji reactions; rich text sanitised on the server; voting when the project turns it on; card aging on by default after 7 days, never for completed cards.                               | Owner; voting default assumed |
| Card fronts       | Labels, title, votes, dates, checklist progress, counts, members, and custom fields set to "show on card".                                                                                                                                                                                                                        | Assumed                       |
| Look and feel     | The clickable prototype v2 in `design/` is the specification for how the product looks and behaves. Where the prototype and this plan differ on data, permissions or limits, this plan wins; on look or interaction, the prototype wins.                                                                                          | Owner                         |
| Card layout       | One wide dialog in two columns that scroll on their own: details on the left; comments and activity on the right with the comment box on top. A top strip holds Card menu and Close. Card menu groups: Add to card, and Actions (move, copy, watch, make template, archive, share link). One column on narrow screens and in zen. | Owner                         |
| Long cards        | Show the most recent few with Show all N / Show fewer: 6 checklist items, 3 links, 3 attachments, 4 comments and activity. Long unnamed links shortened to 30 characters, an ellipsis and the last 10.                                                                                                                            | Owner                         |
| Zen mode          | Toggled with Z or a button. Hides the app header, board header and filter bar on board, table and dashboard. A strip at the top edge holds the controls. Quiet by default. A card opens as a side panel. Stays on until turned off.                                                                                               | Owner                         |
| Keyboard          | Shortcuts act on the selected card or the one under the pointer and are off while typing. Backtick shows the key list; Ctrl K opens the command palette.                                                                                                                                                                          | Owner                         |
| Uploads           | 10 MB per file, per-company storage limit, images and PDFs previewed. Board backgrounds up to 5 MB. Real file type checked, not only the name.                                                                                                                                                                                    | Assumed                       |
| Ordering          | Fractional-index strings; a drag changes one row.                                                                                                                                                                                                                                                                                 | Assumed                       |

## Architecture

A request arrives over HTTPS at Cloudflare, travels through the tunnel to `cloudflared`, and reaches the binary on the loopback address. The session cookie names a person. The URL names a company. The binary checks membership in `control.db`, opens that company's file (cached after first use) and does the work there. Every change also writes an event row in the same transaction; after the commit the binary pushes it to open WebSockets from an in-memory hub, and keeps the last steps for undo.

    [Browser] --HTTPS--> [Cloudflare] --tunnel--> [cloudflared] --127.0.0.1:8791--> [tazko binary web app + /admin · REST /api/v1 + WebSocket · event hub] --opens--> [control.db orgs/<co>/tazko.db files on disk]

#### Data layout on disk

    /var/lib/tazko/data/
      control.db
      secret              random, made on first start
      orgs/
        acme/
          tazko.db
          files/ab/ab12...
      trash/              deleted companies, 30 days

#### Google sign-in

1.  `/auth/google/start` sends the browser to Google with a random state, nonce and PKCE challenge.
2.  `/auth/google/callback` exchanges the code and verifies the ID token signature, audience, expiry and nonce.
3.  Requires `email_verified`. Looks up the person by `sub`, else by email, else a pending invite. Otherwise refuses with a clear message and writes an audit entry.
4.  Creates a session: HTTP-only, Secure, SameSite=Lax cookie, 30 days from last use, stored in `control.db`. State-changing requests also carry a CSRF token. (v2)

#### Workspace and personal accounts

The OAuth consent screen is External and published, which admits any Google account. Invites match on the lower-cased email. Google's `hd` claim is stored only to show which domain an account belongs to.

#### Tenant isolation

No `org_id` column and no policy to forget. A handler gets a handle to exactly one company's file. Backup, export and deletion of a company are file operations.

#### Speed

One writer thread per company owns that file's write connection, plus a few readers. WAL mode, `synchronous=NORMAL`, `foreign_keys=ON`. A busy company never blocks another.

#### Schema changes

Each database file carries its schema version (`PRAGMA user_version`) and migrates when first opened. Control and company databases have separate migration folders.

## Super admin console and invites

| Page           | What it does                                                                                                                                                                                                      |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Companies      | List with member count, boards, storage used and last activity. Create a company (makes its folder and database). Suspend and resume. Delete with a typed confirmation; the folder moves to `trash/` for 30 days. |
| Company detail | Members and roles, pending invites, storage limit. Invite a person as owner or member; change a role; remove a person. A read-only Projects tab lists project names with owners and member and board counts.      |
| People         | Everyone across companies, with their memberships and last sign-in. Disable or re-enable. Grant or revoke super admin.                                                                                            |
| Invites        | Pending invites across all companies; revoke one.                                                                                                                                                                 |
| Audit log      | Who did what and when: company created or deleted, invite created or revoked, role changed, person disabled, sign-in refused, company exported. (v2)                                                              |
| Server         | Version, uptime, data size, last backup and last restore test, email status with a send-test-email button. (v2)                                                                                                   |

**Company owners** get the same member and invite screens for their own company only, plus **Export company**. **An invite** authorizes one email address for one company and role. Until email is set up (M6) the admin sends the person the site address by hand; with email on, the invitation can be emailed from the invite form. When they sign in with that Google account they join automatically. Invites stay valid until revoked.

## Command line

People can work from a terminal: one program, sign in once through the browser, then short commands. The same program, `tazko`, also runs the server on the server machine.

### Signing in

1.  `tazko auth login --host tazko-dev.hugepigstudio.com` asks the server for a one-time device code and prints it.
2.  The browser opens `/cli/authorize`. The signed-in person checks that the code matches, sees the access requested, and approves.
3.  The program receives a personal token, keeps it in the system keychain (or a private file where there is none) and shows who is signed in.
4.  `TAZKO_TOKEN` and `TAZKO_HOST` override this for scripts.

### Conventions

- Nouns, then verbs: `tazko card move`, `tazko board list`.
- Readable tables; `--json fields` and `--jq` for scripts; `--web` opens the item in the browser.
- Exit codes: 0 success, 1 failure, 2 wrong usage, 4 not signed in.
- Shell completion, `tazko update`, `tazko config`.

| Command                        | Subcommands                                                          | What it does                                                         |
| ------------------------------ | -------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `tazko auth`                   | login, logout, status, token                                         | Sign in with the browser, see who you are, print a token for scripts |
| `tazko project`                | list, view, members                                                  | Projects you belong to and who is in them                            |
| `tazko board`                  | list, view, create, open, export, import (v2)                        | Work with boards                                                     |
| `tazko card`                   | list, view, create, edit, move, done, assign, comment, link, archive | Everything on a card from the terminal                               |
| `tazko my`                     | cards, checklist                                                     | Cards and checklist items assigned to you (`--all-companies`)        |
| `tazko search`                 | "query"                                                              | Same operators as the app: `@name`, `#label`, `is:overdue`           |
| `tazko token`, `tazko webhook` | list, create, revoke, test, deliveries                               | Personal tokens; company tokens and webhooks for owners              |
| `tazko api`                    | METHOD /path                                                         | Call any API endpoint and filter the answer with `--jq`              |

## Personal settings, integrations and email

#### Personal settings

- **Profile:** display name, job title, photo from Google or initials, language (English now; Vietnamese later).
- **Appearance:** theme follows the device or is set to light or dark; zoom 80–150% for the whole interface; card density; reduce motion; labels as names or small bars; zen mode switch, density and quiet notices.
- **Date and time:** time zone (with detect), date format, 12 or 24 hour, first day of the week.
- **Notifications:** events by channel (app, email); email delivery instant, hourly, daily or never; reminder lead time; preview of each email.
- **Access tokens and Sessions:** personal tokens; browsers, terminals and phones signed in, each with Sign out.

#### Integrations (company owners)

- **API tokens** (M6): name, access (read, read and write cards, manage), projects, expiry, last used. Secret shown once. Rate limited per token.
- **Webhooks** (M6): address; events card created, updated, moved, completed, archived; comment created; member added; board created; projects; signing secret shown once; pause switch; delivery log with redeliver; test event.
- All reachable from the command line and the REST API.

#### Email

- Set up in the server settings file. The admin Server page shows its status and can send a test.
- Every invite form has an "Email the invitation" option leading to Google sign-in.
- Mention, assignment and due-date emails follow each person's choices. Reminders always appear in the app.
- Emails come from one address and carry no tracking.

## Keyboard, zen mode and card layout

These come from the prototype and are part of the product. Shortcuts are off while typing in a field or editing a description, and while a popover is open.

| Key        | Does                                                                                                     |
| ---------- | -------------------------------------------------------------------------------------------------------- |
| \` or ?    | Show or hide the key list                                                                                |
| Z          | Zen mode on or off                                                                                       |
| Ctrl K     | Command palette                                                                                          |
| /          | Search                                                                                                   |
| F, Q       | Filter; show only my cards                                                                               |
| N          | New card in the selected card's list                                                                     |
| J K H L    | Next or previous card, previous or next list (arrow keys too once a card is selected, and always in zen) |
| Enter, E   | Open the selected card; quick edit                                                                       |
| Space      | Mark done                                                                                                |
| A D T M    | Assign members, dates, labels, move                                                                      |
| V, X       | Vote (when on); archive                                                                                  |
| 1–8        | Toggle the label in that position                                                                        |
| Alt ↑ ↓    | Move a checklist item                                                                                    |
| Ctrl Enter | Save the comment being written                                                                           |
| Esc        | Close what is open, one layer at a time; then leave zen                                                  |

### Command palette

- Opens with Ctrl K or the button in the zen strip. Type to filter; arrows and Enter run the item.
- Groups: Actions (zen, switch view, new card), Filter, Density, Zen quiet mode, Card actions for the selected card. A card action says so when no card is selected.

### Zen mode

- Z, the Zen button, the palette, or Settings → Appearance. A personal setting.
- Density comfortable, compact or minimal. A card opens as a 520 px panel with previous and next. On touch the controls stay visible.

### Cards on the board

- The selected card has a clear outline while keys drive the selection.
- Pasting several lines into the new-card box creates one card per line.
- "Add a card" shows on hover, focus, key selection, an empty list, and always on touch.

## Data model

| File                     | Tables                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `control.db`             | `users` (id, email, google_sub, name, avatar, hd, is_super_admin, disabled, last_seen), `orgs` (id, slug, name, status, storage_limit), `org_members` (user, org, role), `invites` (org, email, company role, optional project, optional project role, created_by, revoked_at), `sessions`, `audit_log`, `user_settings` (theme, zoom, time zone, formats, zen choices, notification choices), `personal_tokens`, `device_codes`, `email_queue` |
| `tazko.db`: structure    | `people` (copy of id, name, avatar), `projects` (name, description, color, default background, members_can_create_boards, members_can_invite, aging_enabled (default on), aging_days (default 7), archived_at, deleted_at), `project_members` (user, role), `boards` (project, name, background, restricted), `board_access`, `lists`, `cards` (is_template (v2)), `labels`, `card_labels`, `card_members`                                      |
| `tazko.db`: card content | `checklists`, `checklist_items` (text, done, assignee, due), `comments`, `comment_reactions`, `card_links`, `card_votes`, `attachments` (content hash, size, type checked), `custom_field_defs` (show_on_card), `custom_field_values`, `cards_fts` (FTS5)                                                                                                                                                                                       |
| `tazko.db`: activity     | `events` (type, actor, board, payload, created_at), `notifications`, `board_templates`, `api_tokens`, `webhooks`, `webhook_deliveries`, `imports` (file name, counts, person mapping, finished_at) (v2)                                                                                                                                                                                                                                         |

## Deployment

Complete enough to follow on a machine that has never seen the project. Target: Ubuntu 24.04 or later, 4 GB of memory, a second disk or folder for backups. The machine is `192.168.86.49` and the hostname is `tazko-dev.hugepigstudio.com`, a name in a domain already on the owner's Cloudflare account.

**The machine is shared.** It already runs another service on `127.0.0.1:8790` and an existing Cloudflare tunnel whose configuration is `/etc/cloudflared/config.yml` with a rule for another hostname. Do not touch those, that service's accounts, or its folders under `/opt`, `/etc` and `/var/lib`. Never rerun a tunnel setup script that rewrites `config.yml`. Tazko only adds a rule. **Every command that changes the machine (sudo, users, systemd, cloudflared) is shown to the owner and waits for approval, in every prompt.**

### Accounts and folders

- `tazko`: system user, no login. Runs the service. Writes only `/var/lib/tazko`.
- `tazko-deploy`: system user. Owns `/opt/tazko` and `/etc/tazko`. Through sudo it may run only `systemctl restart tazko.service` and read that unit's log.
- The administrator runs the release script as `tazko-deploy`.
- `/var/lib/tazko/data` data; `/var/backups/tazko` backups; `/var/lib/tazko-incoming` uploaded bundles.
- Port 8791 on 127.0.0.1 only.

### Settings file

`/etc/tazko/tazko.env`, mode 640, readable by `tazko-deploy`, the `tazko` group and root.

- `TAZKO_HOST=127.0.0.1`, `TAZKO_PORT=8791`
- `TAZKO_PUBLIC_URL=https://tazko-dev.hugepigstudio.com`
- `TAZKO_DATA_DIR=/var/lib/tazko/data`
- `TAZKO_GOOGLE_CLIENT_ID`, `TAZKO_GOOGLE_CLIENT_SECRET`
- `TAZKO_SECRET` (32+ characters; empty means a random `secret` file in the data folder)
- `TAZKO_MAX_UPLOAD_MB=10`
- Email, optional: `TAZKO_SMTP_HOST`, `TAZKO_SMTP_PORT`, `TAZKO_SMTP_USER`, `TAZKO_SMTP_PASSWORD`, `TAZKO_MAIL_FROM`

### 0. Local development (v2)

1.  `cargo run -- serve` listens on `127.0.0.1:8791`; the web app runs with hot reload from `web/`.
2.  Use a separate Google OAuth client with redirect `http://localhost:8791/auth/google/callback` (Google allows plain HTTP for localhost).
3.  `tazko admin --data ./data --email you@example.com` creates a local super admin. `tazko dev seed` loads the prototype's sample companies and boards.
4.  Practice builds and anyone other than the owner use only this target, never the shared machine.

### 1. Install and configure the tunnel (adds a rule, keeps the existing one)

1.  Read `/etc/cloudflared/config.yml` and note the tunnel name or id and the existing rules. `cloudflared` is already installed if that file exists.

2.  Insert a rule above the final catch-all `- service: http_status:404`:

        - hostname: tazko-dev.hugepigstudio.com
          service: http://127.0.0.1:8791
          originRequest:
            connectTimeout: 30s
            noHappyEyeballs: true

3.  Create the DNS record against that tunnel with the account certificate already on the machine: `sudo env TUNNEL_ORIGIN_CERT=/etc/cloudflared/cert.pem cloudflared tunnel route dns <existing tunnel name> tazko-dev.hugepigstudio.com`. If the certificate is missing, run `cloudflared tunnel login` and have the owner open the printed link.

4.  Check with `cloudflared tunnel ingress validate --config /etc/cloudflared/config.yml`, then `sudo systemctl restart cloudflared`, and confirm the unit is active and the old hostname still works.

5.  If no tunnel exists, install `cloudflared` from Cloudflare's apt repository, run it as its own user, and write a new config with the same rule plus the 404 catch-all.

### 2. The service

    [Unit]
    Description=Tazko
    After=network.target

    [Service]
    User=tazko
    Group=tazko
    EnvironmentFile=/etc/tazko/tazko.env
    ExecStart=/opt/tazko/current/tazko serve
    Restart=on-failure
    RestartSec=2
    NoNewPrivileges=true
    ProtectSystem=strict
    ProtectHome=true
    PrivateTmp=true
    ReadWritePaths=/var/lib/tazko
    CapabilityBoundingSet=
    SystemCallArchitectures=native

    [Install]
    WantedBy=multi-user.target

### 3. Releases, deploy and rollback

1.  Build on the server: `cargo build --release --target x86_64-unknown-linux-musl`. The bundle is one executable plus a `VERSION` file, named `tazko-<time>-<commit>.tar.gz`.
2.  Copy it to `/var/lib/tazko-incoming`, then run the release script as `tazko-deploy` (`/usr/local/bin/tazko-release`, root-owned).
3.  The script unpacks into `/opt/tazko/releases/<time>-<commit>`, points `/opt/tazko/current` at it, and runs `sudo systemctl restart tazko.service`.
4.  It polls `http://127.0.0.1:8791/health` for up to 20 seconds. If unhealthy it points the link back, restarts and exits with an error. Options `--rollback` and `--list`. Keeps the last 5 releases.

### 4. First super admin

    sudo -u tazko /opt/tazko/current/tazko admin \
      --data /var/lib/tazko/data --email trungngo@hugepigstudio.com

### 5. Google sign-in (the owner does this in a browser)

1.  In the Google Cloud console create a new project named **Tazko**.
2.  OAuth consent screen (Google Auth platform): audience **External**, app name Tazko, support email, authorized domain `hugepigstudio.com`, scopes `openid`, `email`, `profile` only.
3.  **Publish the app** (Testing → In production). In Testing only listed users can sign in.
4.  Create an OAuth client ID of type **Web application** with redirect URI `https://tazko-dev.hugepigstudio.com/auth/google/callback`. No JavaScript origin is needed.
5.  Put the client ID and secret into `/etc/tazko/tazko.env` as root and restart. Never put them in the repository or a chat.

### 6. Cloudflare settings

- No Cloudflare Access application: Tazko does its own sign-in.
- Rate limiting rule on URI path starting with `/auth/` and `/api/`, for example 20 requests per 10 seconds per IP on `/auth/`. (v2)

### 7. Backups and restore (v2)

1.  **Nightly:** `tazko-backup.timer` at 02:30 runs `tazko backup --data /var/lib/tazko/data --to /var/backups/tazko --keep-days 30`: online-backup copies of every database, the `files/` folders, dated snapshots.
2.  `tazko check` on the copy: opens every database, runs `PRAGMA integrity_check`, verifies every attachment against its hash.
3.  **Weekly restore test:** restores the newest nightly snapshot into a scratch folder, runs `tazko check`, and records the result on the admin Server page.
4.  Restore: stop the service, `tazko backup restore --from /var/backups/tazko --to /var/lib/tazko/data.new`, `tazko check` it, move `data` to `data.old` and `data.new` to `data`, start the service.

## Milestones

Each milestone ships something usable. A milestone is finished only when every "Done when" check passes and its prototype v2 screens match.

### M1 Platform: Deployed, signed in, companies managed

#### Tasks

- Cargo project and `web/` app (React, Tailwind, shadcn/ui); build step embeds `web/dist` (disk reload in development)
- Config from environment; commands `serve`, `admin --email`, `backup`, `backup restore`, `check`, `dev seed`
- `control.db`; open, cache and migrate company databases; writer thread and read pool per company
- Google sign-in as described above; sessions with CSRF tokens; sign-out; clear refusal page
- Super admin console: companies, company detail, people, invites, audit log, server page
- Company owner screens: members and invites for their own company; company switcher; empty company home
- Everything in the Deployment section as scripts and units under `deploy/`, including the nightly backup and the weekly restore test (v2)
- Local development target and `tazko dev seed` (v2)
- `scripts/ci.sh` including a dependency licence check (MIT, Apache-2.0, BSD, ISC only)
- README deploy guide that repeats the Deployment section

#### Done when

- The site loads through the tunnel with no new port open, and the other service on the machine still works
- `tazko admin --email trungngo@hugepigstudio.com` then Google sign-in reaches `/admin`
- A Workspace account and a personal Gmail account can both be invited and both sign in
- An account with no person and no invite is refused and the refusal is in the audit log
- A company can be created, suspended, resumed and deleted (folder lands in `trash/`)
- A person in two companies sees only the chosen company's data (test)
- Deploying a broken build rolls back by itself
- The weekly restore test restores the newest nightly backup, passes `tazko check`, and its result shows on the admin Server page (v2)
- The local target runs end to end with `tazko dev seed` (v2)

### M2 Core boards: One team can run a real board, and move its boards in

#### Tasks

- Boards, lists, cards: create, rename, archive, restore; star a board; fractional ordering; drag and drop of cards and lists
- Card modal: markdown and rich-text description (sanitised on the server), assignees, labels, dates and done, colour cover
- Checklists with progress badge and inline expand; add and reorder checklists; comments; calm tile badges (no votes or fields unless enabled)
- Manage projects for company owners; project settings (name, description, colour, default background); project members (invite, roles, remove)
- Restricted boards; projects list per person
- Search with FTS5 and operators (`@member`, `#label`, `is:overdue`, `has:attachments`, `is:archived`); filter by member, label, due date
- Card actions (move, copy, archive and restore, watch, share link, quick edit, complete circle); list actions (rename, collapse, copy, move, sort, move all, archive all, archive); board menu
- **Import from a board export file** with a preview, person matching by email then name, archived cards and comments optional; **export a board** and, for owners, **a company** as JSON (v2)
- Personal settings (profile, appearance with whole-UI zoom, date and time, sessions)
- Event row for every change; activity log
- Card dialog in two scrolling columns; long cards; zen mode on Z; key map; multi-line paste; "Add a card" rule

#### Done when

- A team runs a real sprint board end to end
- A project observer cannot edit or comment; a company member not in a project cannot see it; a company owner can archive and delete it and add owners but cannot open its boards until added (tests)
- A company member who is not an owner is refused when creating a project (test)
- Inviting a new email to a project lets them sign in, join the company, and see only that project (test)
- A restricted board is hidden from project members who were not added (test)
- Moving 200 cards creates no rank collisions (test)
- Importing an export file of 124 cards keeps lists, labels, checklists, dates, comments and people; exporting it again and importing into another company gives the same board (test) (v2)
- Description HTML is sanitised on the server (test)
- Changing theme, zoom or time zone applies at once and is still set after signing in on another device
- With the card open the comment box is visible without scrolling and the columns scroll separately; Card menu and Close never cover comments
- Pressing Z hides the header, board header and filter bar on board, table and dashboard; Z or Esc restores them
- Every shortcut in the key map works and none fires while a field is being edited

### M3 Live and files: Feels like a shared board

#### Tasks

- WebSocket per board with membership check; in-memory hub; optimistic UI and catch-up by event id; presence avatars
- Attachment uploads into the company's `files/` folder by content hash; 10 MB per file; real type check; link attachments; thumbnails; company storage limit; image covers
- @mentions; notification bell with read state; due-soon alerts
- Custom fields: definitions per board, values on cards, "show on card" switch, filter by value
- Reactions on comments; uploaded board backgrounds (up to 5 MB)

#### Done when

- Two browsers on one board see each other's drags within a second
- A dropped connection reconnects and shows missed changes
- A mentioned user gets a notification and opens the card from it; watching a board notifies about new cards on it
- An upload past 10 MB or the company's limit is refused with a clear message; a file renamed to look like an image but isn't is refused (v2)

### M4 Views and templates: Repeat the same structure across companies

#### Tasks

- Board templates: save a board, create a board from it; a super admin template can be offered to every company
- Card templates: built-in ones plus **"Make template" on any card**, listed under the template button of every list (v2)
- Table view: one row per card, sortable columns, group by list, show or hide columns (custom fields become columns), edit in place, bulk move, assign and archive, copy as CSV
- My cards: assigned cards, assigned checklist items and watched cards, grouped by due date, with project and title filters, hide completed, and an **All companies** switch (v2)
- Dashboard view: cards per list, member, label and due date; Most votes when voting is on
- Command palette with groups and card actions

#### Done when

- Ctrl K opens the palette; typing narrows it; Enter runs the item; a card action acts on the selected card and says so when none is selected
- A template creates an identical board in another company
- A card made into a template creates new cards with its checklists and labels, and stops being offered when unmarked (v2)
- Sorting and grouping in the table never change card order on the board
- My cards lists only projects the person belongs to; with All companies on it includes their other companies and opening a card switches company (v2)
- Editing a due date in the table updates the card on the board and appears in its activity
- Dashboard totals equal the counts on the board view

### M5 Card depth: Richer cards for planning work

#### Tasks

- Checklist items: optional assignee and due date, shown only when set; assign and date from a small menu on hover; assigned items appear in My cards and notify the assignee
- Recurring cards: daily, every weekday, weekly, every 2 weeks or monthly; completing reopens with the next due date and clears the checklist, with undo
- Card links: relates to, blocks, is blocked by, duplicates; shown on both cards; an open blocker shows a Blocked badge
- Voting: one vote per person, count on the tile, voter avatars, vote from tile, card, quick menu or V; sort by votes; Votes column; Most votes chart
- Card aging: switch and threshold (3, 7, 14 or 30 days), on by default at 7; three fade steps; completed cards excluded; Stale filter, `is:stale`, Stale count, Last activity column

#### Done when

- A checklist item with an assignee appears in that person's My cards with its due date
- Completing a weekly card moves its due date forward a week and unticks its checklist; undo restores both
- Removing a link removes it from both cards; completing the blocking card clears the Blocked badge
- Voting twice removes the vote; tile, table and dashboard agree
- With aging on, a quiet card fades and a comment restores it; turning the project setting off makes every card full strength at once (tests)

### M6 Integration and email: Connect other tools, work from a terminal, get email

#### Tasks

- **REST API** under `/api/v1` with an OpenAPI document and reference at `/api/docs`; **personal tokens** and **company API tokens** with rate limits
- Webhooks for owners: events and projects, HMAC signature, retries with growing delay for a day, delivery log, redeliver, test event, pause
- Command line tool: device-code sign-in with the `/cli/authorize` approve screen, command groups, `--json` and `--jq`, install scripts and downloads at `/cli`
- Email, optional (SMTP in the settings file): "Email the invitation" on every invite form; mention and assignment emails; due-date reminders; instant, hourly or daily delivery; previews in Settings; status and test on the admin Server page
- Notification preferences per person, in the app and by email, for each event type

#### Done when

- `tazko auth login` signs in through the browser and `tazko card create` works from a terminal on another machine
- A token limited to one project cannot read another project; a token over its rate limit gets 429 (test)
- A webhook delivery carries a signature that verifies with the secret; a failing endpoint is retried and visible in the log
- With email on, an invited person gets an email that leads to Google sign-in; with email off nothing breaks and the invite shows the address to share
- Someone who turns off mention emails still sees mentions in the app

## Security checklist (v2)

#### Requests

- CSRF token on every state-changing request, in addition to SameSite=Lax cookies.
- Rate limits: sign-in endpoints per IP, API per token.
- Every handler checks project role and board access on the server; the UI only hides buttons.

#### Content

- Rich text sanitised on the server with an allow-list.
- Uploads: size limit, real type from the file's bytes, served with `Content-Disposition` and no sniffing; images re-encoded for thumbnails.
- Imported files parsed with size and depth limits.

#### Secrets and people

- Tokens stored only as hashes and shown once; webhook and email-to-board secrets can be rotated.
- Secrets never in the repository, logs or chat.
- Removing a person from a company ends their sessions and tokens there at once; their name stays on past activity.

## Clean-room and commit rules

Tazko is an original work. These rules apply to every file, commit and message the build session produces.

#### Commits and pull requests

- Never mention AI, assistants or code-generation tools. No co-author trailer naming one, no "generated with" line, no session or tool links.
- The author is the owner's own git identity. Disable the tool's automatic commit and pull-request attribution in the repository's `.claude/settings.json` (set `attribution.commit` and `attribution.pr` to empty strings) and check the first commit message.
- Messages describe the change in plain words.

#### No other products by name

- Never name any other product, vendor or competitor in code, comments, identifiers, strings, tests, fixtures, sample data, documentation, commit messages, branch names, package names or UI text.
- The importer reads "a board export file"; describe its format by its fields, not by a product.
- If something seems to need a comparison with another product, ask the owner.

#### Original work only

- Do not copy another product's code, text, icons, colours, layout assets, templates, help copy or sample content.
- Do not capture, scrape or reverse engineer another product's pages, scripts, API or network traffic. Work only from this plan and the prototype.
- Use only dependencies under permissive licences and keep the licence check in CI.

## The prototype is the specification

The owner wants the finished product to look and behave like the clickable prototype. Version 2 is the reference for M1 to M6; a milestone is not finished until its screens match.

### What lives in `design/`

- `design/tazko-prototype.html`: prototype v2 in one file. Start from the index page; the two "v2" groups at the bottom open each change.
- `design/tokens.css`: colours, spacing, radii, shadows and type for light and dark, to be turned into the Tailwind theme.
- `design/README.md`: the screen index below and how to use the prototype.
- `design/CHANGELOG.md`: D1 (first prototype from anh Trung), D2 (changes from the review).
- Read-only in the build; ask the owner to change it.

### How the build uses it

- Before each milestone, open the matching tiles and note layout, states (empty, loading, error, read-only), wording, icons and keyboard behaviour.
- Recreate each screen with real components. Do not copy the prototype's scripts; the CSS values, icon shapes and wording may be reused.
- The View as switch shows what each role sees. Build the same differences, enforced on the server.
- Things the prototype only pretends: sample data, live updates, email, uploads, the command line, undo history across sessions. Build them as this plan describes.
- Prototype-only controls are never built: the dark prototype bar, View as, Live sim.

### Screen index by milestone

| Milestone | Prototype v2 screens                                                                                                                                                                                        |
| --------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M1        | Sign in, Sign-in refused, Company home, Company settings, Manage projects, Admin: Companies, Company detail, People, Invites, Audit log, Server                                                             |
| M2        | Your projects, Project boards, Project members, Project settings, Board, Card, Filter, Search, Zen mode, Long cards, Rich text, Import a board, Board menu → Export board, Settings: profile and appearance |
| M3        | Attachments (10 MB), Custom fields, Live updates, Notifications, Reactions, Board background upload                                                                                                         |
| M4        | Templates, Card menu → Make template, Table view, Dashboard, My cards (All companies), Command palette                                                                                                      |
| M5        | Checklist items, Recurring cards, Linked cards, Voting, Card aging                                                                                                                                          |
| M6        | Settings: notifications, access tokens and command line, Authorize CLI, Integrations: API tokens and webhooks, Email previews, Admin: email status                                                          |

## Repository and handoff

The build session reads everything from the repository: `ralts/tazko` on the company GitLab. It builds one milestone at a time and stops for the owner to check, unless the owner sends the build-all prompt.

### Layout

    tazko/
      CLAUDE.md
      README.md            deploy guide
      design/              prototype v2, tokens, index, changelog
      docs/
        build-plan.html    this plan
        HANDOFF.md
        prompt-*.txt
        progress.md        written by the build
        open-questions.md  written by the build
      Cargo.toml
      build.rs             builds or checks web/ and embeds web/dist
      src/
        main.rs            CLI: serve, admin, backup, check, dev
        config.rs  auth/  admin/  orgs/  tenant/  boards/  cards/
        realtime/  files/  notifications/  api/  import/  mail/
      migrations/  control/  tenant/
      web/                 React + TypeScript + Vite + Tailwind + shadcn/ui
      deploy/              setup-host.sh add-tunnel-rule.sh release.sh
                           build-bundle.sh tazko.service
                           tazko-backup.service tazko-backup.timer
                           tazko-restore-test.service tazko-restore-test.timer
      tests/
      scripts/ci.sh

### CLAUDE.md changes for v2

- Work one milestone at a time (M1 to M6) in plan order, unless told to build all.
- The prototype in `design/` is v2; read `design/CHANGELOG.md` D2 for what changed.
- Import and export use the generic board export format; never name the source product.
- Uploads: real file type check.
- **Every command that changes the machine needs the owner's approval**, in every prompt, including full run and build all.
- Practice builds use the local target only.
- Keep the existing rules: data layout, writer thread, sign-in, access model, secrets, clean-room, look and feel from `design/`, `scripts/ci.sh` before every commit, small commits, test every permission rule, ask when ambiguous.

### Prompts

| File                        | When                  | What it asks                                                                                                                                                                                                        |
| --------------------------- | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `prompt-kickoff.txt`        | First message         | Build M1 only, from this plan and the M1 screens; show every machine-changing command and wait.                                                                                                                     |
| `prompt-next-milestone.txt` | Once per milestone    | Open the milestone's prototype screens, build every task with tests for every "Done when", run `scripts/ci.sh`, stop with screenshots next to the prototype screens.                                                |
| `prompt-full-run.txt`       | After M1 passes       | Build M2 to M6 in order; stop only for a failing check, a plan-versus-prototype conflict, or a machine-changing command (still needs approval). (v2)                                                                |
| `prompt-build-all.txt`      | One-shot run          | One round of questions first, then M1 to M6 without stopping; assumptions in `docs/open-questions.md`; final report comparing every screen with the prototype. Machine changes only if approved in the first round. |
| `prompt-design-update.txt`  | After a new prototype | Pull, read the new CHANGELOG entries since the version in `docs/progress.md`, change only those screens, note pending entries for unbuilt milestones.                                                               |

### Message to send for this version

    Run git pull. Read CLAUDE.md, docs/HANDOFF.md and docs/build-plan.html (plan v2) and
    design/CHANGELOG.md entry D2. Prototype v2 in design/ is the visual reference.
    Then follow docs/prompt-kickoff.txt for M1, or docs/prompt-design-update.txt if
    M1 or later is already built. Machine-changing commands always need my approval.

## Deferred features to revisit

### Still deferred

- Desktop app (Tauri)
- Production host and cloud deployment
- Vietnamese interface (strings are already in an i18n file)
- A card on several boards (mirror); links cover most cases today

### Not built, worth a decision

- A guest role for people outside the company
- Who can archive cards, as a project switch
- Read-only public board links (leaning against, for privacy)
- Time tracking on cards

## Assumptions and open items

**Assumed in v2, not yet confirmed by the owner.** Correct any of them and the plan will be updated.

| \#  | Assumption                                                                                                  |
| --- | ----------------------------------------------------------------------------------------------------------- |
| 1   | Import and export are in M2.                                                                                |
| 2   | No guest role; someone invited only to a project joins as a member who sees only that project.              |
| 3   | My cards shows the current company with an All companies switch.                                            |
| 4   | Nightly backup as in v1, plus a weekly restore test shown on the admin Server page.                         |
| 5   | Practice builds run on localhost only.                                                                      |
| 6   | Still assumed from v1: web stack, role names, company-wide webhooks, 5 MB backgrounds, fractional ordering. |

### Decided

- Hostname `tazko-dev.hugepigstudio.com`
- Machine `192.168.86.49`, shared, port 8791
- First super admin `trungngo@hugepigstudio.com`
- New Google Cloud project named Tazko, created by the owner
- Repository `ralts/tazko` on the company GitLab

### Still to find out

- Which in-house tool should be the first real integration (it would use the REST API in M6).
- On the server: the tunnel's name and current rules, the installed cloudflared version, free disk space for backups.

Tazko build plan v2 · 16 Nov 2026 · Based on the first build plan and the reviews of 13 and 16 Nov. Visual reference: Tazko Prototype v2.

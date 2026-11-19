# Tazko: rules for the build

Tazko is a self-hosted kanban work tracker for several companies: one Rust binary with the web app embedded, one SQLite file per company, Google sign-in, served through a Cloudflare Tunnel. The full plan is `plan.md`; the look and behaviour are in `design/`. Read both before starting a milestone.

## How to work

- Build **one milestone at a time**, M1 to M6 in plan order. Stop when a milestone's "Done when" checks pass and its screens match the prototype, and wait for the owner. Build several only when the owner sends the build-all prompt.
- Before a milestone, open its screens in `design/tazko-prototype.html` (index in `design/README.md`). Where plan and prototype differ on data, permissions or limits, the plan wins; on look or interaction, the prototype wins.
- `design/` is read-only. Ask the owner to change it. `design/CHANGELOG.md` lists what changed per prototype version (D2 is v2).
- Write a test for every "Done when" check and for every permission rule.
- Small commits. Run `scripts/ci.sh` before every commit; do not commit while it fails.
- No comments in `src/`, `web/src/` or `migrations/`: name things so the code reads on its own, and put reasons in the commit message or `docs/`.
- When the owner corrects the same mistake twice, turn it into a check in `scripts/checks.py` with a test, not into another line here.
- When something is ambiguous, ask the owner; record unanswered questions in `docs/open-questions.md` and progress in `docs/progress.md`.

## Data layout

- `control.db`: people, companies, company members, invites, sessions, audit log, personal settings, personal tokens, device codes, email queue.
- One `orgs/<slug>/tazko.db` per company for everything else, with that company's files under `orgs/<slug>/files/` by content hash. No `org_id` column: a handler gets a handle to exactly one company's file.
- Deleted companies move to `trash/` for 30 days.
- SQLite in WAL mode, `synchronous=NORMAL`, `foreign_keys=ON`. One writer thread per company owns its write connection, plus a small read pool.
- Each file carries its schema version in `PRAGMA user_version` and migrates on first open; `migrations/control/` and `migrations/tenant/` are separate.
- Every change writes an event row in the same transaction. Ordering uses fractional-index strings; a drag changes one row.

## Access model

- Sign-in is Google only, invite-only. An account signs in only if its verified email belongs to a person or a pending invite; after that people are matched by Google `sub`. Refusals are audit-logged.
- Super admin is a flag set from the command line. Admins manage companies and people, see project names and counts, never boards. Every admin action is audit-logged.
- Company roles: owner and member. Only owners create projects, choose project owners, and archive or delete projects. An owner opens a project's boards only after adding themselves (audit-logged).
- Project roles: owner, member, observer (read only). Restricted boards are visible only to people added to them. No guest role: someone invited to one project joins the company as a member who sees only that project.
- Every handler checks company membership, project role and board access **on the server**. The UI only hides buttons.
- Personal and company tokens never exceed the access of the person or the projects they were given.

## Deployment target and secrets

- Practice builds and anyone other than the owner use **only the local target** (`cargo run -- serve` on `127.0.0.1:8791`).
- The shared server (`192.168.86.49`) already runs another service on `127.0.0.1:8790` and an existing tunnel. Never touch that service, its accounts or folders, and never rerun a tunnel setup that rewrites `/etc/cloudflared/config.yml`; Tazko only adds a rule.
- **Every command that changes a machine (sudo, users, systemd, cloudflared, packages) is shown to the owner and waits for approval**, in every prompt, including full run and build all.
- Secrets (Google client secret, SMTP password, `TAZKO_SECRET`, tokens) live only in `/etc/tazko/tazko.env` or the local untracked `.env`. Never in the repository, logs, test fixtures or chat. Tokens are stored only as hashes and shown once.

## Clean-room and commit rules

- Tazko is an original work. Never name any other product, vendor or competitor in code, comments, identifiers, strings, tests, fixtures, sample data, docs, commit messages, branch names, package names or UI text. The importer reads "a board export file", described by its fields.
- Do not copy another product's code, text, icons, colours, templates or help copy, and do not scrape or reverse engineer another product. Work only from `plan.md` and `design/`.
- Only dependencies under MIT, Apache-2.0, BSD or ISC licences; keep the licence check in CI.
- Commits and pull requests never mention AI, assistants or code-generation tools: no co-author trailer, no "generated with" line, no session or tool links. The author is the owner's own git identity. `.claude/settings.json` turns automatic attribution off; keep it that way. Messages describe the change in plain words.

## Web app size

- The shipped JavaScript in `web/dist` stays **under 200 KB** (gzip-compressed total, checked by `scripts/ci.sh`). Split by route and prefer small dependencies.

## Checks

`scripts/ci.sh` runs the repository checks and their tests, then the Rust and web checks. The repository checks are: required files, clean-room names, secrets, commit authorship and messages, no comments in source, no company column in the company file migrations, no prototype-only controls in the web app, `design/` unchanged against `scripts/design.sha256`, `.claude/settings.json` keeping attribution off and machine commands on ask, and the JavaScript budget. Without `TAZKO_OWNER_EMAIL` set, the commits check accepts only the owner's email as stored by hash in `scripts/checks.py`, not the local git identity. After an approved design change the owner runs `scripts/checks.py design --update`. To check messages as you commit, run `git config core.hooksPath scripts/hooks` once.

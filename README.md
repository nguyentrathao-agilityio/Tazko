# Tazko

A self-hosted kanban work tracker for several companies. One Rust binary with the web app embedded, one SQLite file per company, Google sign-in by invitation only, and a super admin console to run the companies. It is served on `127.0.0.1` and reached through a tunnel, with no open port.

## Status

Built milestone by milestone from [`plan.md`](plan.md):

| Milestone | What | State |
| --- | --- | --- |
| M1 Platform | Binary, sign-in, super admin console, companies, invites, backups, deploy | Accepted on the local target |
| M2 Projects and boards | Projects, boards, lists, cards, card dialog, search, keyboard, settings | Accepted |
| M3 Live and files | Live updates, attachments, custom fields, notifications, reactions | Checked; owner's team trial open |
| M4 Views and templates | Table view, my cards, templates, dashboard, command palette | In progress |
| M5 Card depth | Checklist assignees, recurring cards, links, voting | Not started |
| M6 Integration and email | Webhooks, command line tool, email, REST API and tokens | Not started |

Each step, with how its checks were verified, is in [`docs/progress.md`](docs/progress.md).

## Quick start

You need Rust, Node.js with `pnpm`, `sqlite3`, Python 3 and `cargo-deny`; versions and install commands are in [`docs/dev.md`](docs/dev.md).

```bash
scripts/dev-env.sh check
scripts/dev-env.sh init
cargo run -- admin --data ./data --email you@example.com
cargo run -- dev seed
cargo run -- serve
```

`init` creates `.env.local` (git-ignored); put the local Google OAuth client in it first, as described in [`docs/dev.md`](docs/dev.md#google-sign-in-for-localhost). Then open `http://localhost:8791` and sign in with the address you made super admin. `dev seed` adds a demo company with a sprint board.

For hot reload of the web app, keep the server running and start `pnpm dev` in `web/`, then open `http://localhost:5173`.

## Command line

| Command | What it does |
| --- | --- |
| `tazko serve` | Run the web server |
| `tazko admin --email <address>` | Make a person a super admin |
| `tazko dev seed` | Add the demo company (local addresses only) |
| `tazko backup` | Snapshot the data folder and check it; `restore` and `verify` subcommands |
| `tazko check` | Check every database and stored file in a data folder |

`tazko <command> --help` lists the options.

## How it is built

- `src/`: the server (axum). Every handler checks company membership, project role and board access on the server.
- `web/`: the web app (React, TypeScript, Vite). The shipped JavaScript stays under 200 KB gzip.
- `migrations/control/` and `migrations/tenant/`: schema for `control.db` and for each company's `orgs/<slug>/tazko.db`. Committed migrations never change.
- `design/`: the prototype and design tokens, read-only. Look and behaviour follow it.
- `deploy/`: setup script, systemd units, release with rollback, bundle script.
- `tests/`: Rust integration tests; `tests/ci/`: tests for the repository checks.
- `scripts/ci.sh`: every check; it must end with `ci: ok` before a commit.

## Documents

| File | What |
| --- | --- |
| [`plan.md`](plan.md) | The full build plan: decisions, architecture, data model, milestones, security checklist |
| [`design/README.md`](design/README.md) | Prototype screen index |
| [`docs/dev.md`](docs/dev.md) | Local development: tools, settings, sign-in, run, reset |
| [`docs/deploy.md`](docs/deploy.md) | Install, release, roll back and back up on a server |
| [`docs/progress.md`](docs/progress.md) | What was built per step and how it was verified |
| [`docs/open-questions.md`](docs/open-questions.md) | Questions waiting for the owner |
| [`CLAUDE.md`](CLAUDE.md) | Rules for agents building the project |

## Contributing

Work on a branch from `dev` and open a pull request into `dev`. Run `scripts/ci.sh` before every commit, and follow [`.claude/skills/verify-milestone/SKILL.md`](.claude/skills/verify-milestone/SKILL.md) before every pull request. Dependencies must be MIT, Apache-2.0, BSD or ISC licensed.

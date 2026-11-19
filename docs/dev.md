# Local development

The local target is the only place to practise. It runs on this computer at `http://localhost:8791`, bound to `127.0.0.1`, with no public hostname and no tunnel. Never point a practice build at the shared server.

## Tools

| Tool | Version checked on setup | How to install |
| --- | --- | --- |
| Rust stable (`cargo`, `rustc`, `rustfmt`, `clippy`) | 1.100.0 | `rustup` (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh -s -- --profile minimal -c rustfmt -c clippy`) |
| `cargo-deny` (licence check) | 0.21.0 | `cargo install --locked cargo-deny` |
| Node.js LTS (22 or later, even major) | 24.15.0 | `nvm install --lts` |
| `npm` | 11.12.1 | ships with Node |
| `pnpm` | 11.1.1 | `corepack enable pnpm` or `npm install -g pnpm` |
| `sqlite3` | 3.43.2 | ships with macOS; `apt install sqlite3` on Ubuntu |
| Python 3 | 3.12.0 | runs `scripts/checks.py` |

Check everything at once:

```bash
scripts/dev-env.sh check
scripts/dev-env.sh versions
```

`scripts/ci.sh` runs the same check and its tests on every run.

## Checks

`scripts/ci.sh` must pass before every commit. It runs:

- repository checks: required files, no other product named, no secrets or settings files tracked, commit authorship and messages, the 200 KB JavaScript budget, and the tests for these checks;
- the local tool check and its tests;
- web: `pnpm install --frozen-lockfile`, lint, type check, unit tests, build, the JavaScript budget on the build, and dependency licences;
- Rust: `cargo fmt --check`, `cargo clippy` with warnings as errors, `cargo test`, and `cargo deny check licenses bans sources`.

Allowed licences are MIT, Apache-2.0, BSD and ISC. Exceptions are named one by one: in `deny.toml` for Rust crates, and in `BUILD_TOOL_EXCEPTIONS` in `scripts/checks.py` for web build tools that never ship.

The commits check compares every commit with the owner's git identity. In a fresh clone, set it first, or pass it for one run:

```bash
git config user.name "Owner Name"
git config user.email owner@example.com
TAZKO_OWNER_NAME="Owner Name" TAZKO_OWNER_EMAIL=owner@example.com scripts/ci.sh
```

## Settings

Settings live in `.env.local` at the repository root. It is git-ignored and must stay that way (`git check-ignore .env.local` prints the file name).

```bash
scripts/dev-env.sh init
```

This copies `.env.example` to `.env.local` with owner-only permissions and never overwrites an existing file. The local values are:

- `TAZKO_HOST=127.0.0.1`, `TAZKO_PORT=8791`
- `TAZKO_PUBLIC_URL=http://localhost:8791`
- `TAZKO_DATA_DIR=./data`
- `TAZKO_GOOGLE_CLIENT_ID`, `TAZKO_GOOGLE_CLIENT_SECRET`: from the localhost OAuth client below
- `TAZKO_SECRET`: leave empty to use a random `secret` file in the data folder
- Email settings: leave empty; mail is not sent locally

Secrets go only into `.env.local`. Never paste them into the repository, a commit, a test fixture, a log or a chat.

How `tazko serve` finds its settings, first match wins:

1. `--env-file <path>`
2. the file named by `TAZKO_ENV_FILE`
3. `.env.local` in the current folder
4. `/etc/tazko/tazko.env`

`TAZKO_*` variables already set in the environment override the file. `TAZKO_PUBLIC_URL`, `TAZKO_DATA_DIR`, `TAZKO_GOOGLE_CLIENT_ID` and `TAZKO_GOOGLE_CLIENT_SECRET` are required; the server refuses to start and lists every missing or invalid value. Email is optional, but `TAZKO_SMTP_HOST` and `TAZKO_MAIL_FROM` are both needed once any email value is set. `TAZKO_SECRET`, when set, must be at least 32 characters.

On start the server creates, when missing, the data folder (owner-only), `control.db`, `orgs/` and `trash/`, and the `secret` file with mode 600. An existing `secret` file readable by anyone else stops the start with a message to `chmod 600` it. Secret values are never printed; the start-up line shows only the address, public URL, data folder, settings file and whether email is on.

## Google sign-in for localhost

Use a separate OAuth client for local work, never the one for the shared server.

1. In the Google Cloud console, open the Tazko project, then **Google Auth platform → Clients → Create client**.
2. Application type **Web application**, name `Tazko local`.
3. Authorized redirect URI: `http://localhost:8791/auth/google/callback`. No JavaScript origin is needed. Google accepts plain HTTP for `localhost`.
4. Copy the client ID into `TAZKO_GOOGLE_CLIENT_ID` and the client secret into `TAZKO_GOOGLE_CLIENT_SECRET` in `.env.local`.
5. While the consent screen is in testing, add the Google accounts you sign in with as test users.

Open the app at `http://localhost:8791`, not `http://127.0.0.1:8791`, so that the redirect matches.

## Run

```bash
cargo run -- serve
```

The server listens on `127.0.0.1:8791` and serves the web app built into the binary; `http://localhost:8791/health` returns the version. `cargo build` runs `pnpm install` (first time only) and `pnpm run build` in `web/`, then embeds `web/dist` brotli-compressed. Set `TAZKO_SKIP_WEB_BUILD=1` to embed the current `web/dist` without rebuilding it.

For hot reload while working on the web app, keep the server running and start Vite in a second terminal:

```bash
cd web && pnpm dev
```

Open `http://localhost:5173`; `/health`, `/api` and `/auth` are passed through to the server.

Web app commands, from `web/`:

| Command | What it does |
| --- | --- |
| `pnpm dev` | Vite with hot reload |
| `pnpm run lint` | Lint with warnings as errors |
| `pnpm run typecheck` | TypeScript check |
| `pnpm test` | Unit tests |
| `pnpm run build` | Build `web/dist` |

A release build is one file with the web app inside:

```bash
cargo build --release
```

The binary is `target/release/tazko`; copy it anywhere and run `tazko serve`.

## Sign in locally

Sign-in is invite-only, so create a local super admin with your Google address first, then sign in with that account:

```bash
cargo run -- admin --data ./data --email you@example.com
```

Load a demo company ("Tazko Demo") with a project, a "Sprint 12" board of about fifteen cards (labels, dates, checklists, comments, some Vietnamese text) and a "Roadmap" board, owned by the super admin:

```bash
cargo run -- dev seed
cargo run -- dev seed --owner you@example.com
```

It goes through the app's own routes, so events, ordering, search and the description cleaner all apply. It runs only when `TAZKO_PUBLIC_URL` is a local address, and does nothing if the demo company already exists. Stop the server first, or run it while the server is stopped.

## Reset data

All local data is under `./data` (`control.db`, `orgs/`, `trash/`, the `secret` file). It is git-ignored. To start over, stop the server and remove it, then create the admin and seed again:

```bash
rm -rf ./data
```

`.env.local` is kept, so the OAuth client does not need to be set up again.

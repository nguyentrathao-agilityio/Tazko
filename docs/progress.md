# Progress

One entry per step: what was built, how its checks were verified, and anything left open. Newest last.

| Step | Date | Built | Checks and how they were verified | Open |
| --- | --- | --- | --- | --- |
| 0.4 | 16 Nov 2026 | Seed: plan, prototype v2, rules, repository checks (PR #1). | `scripts/ci.sh` green. | |
| 0.5 | 17 Nov 2026 | Local target: `.env.example`, `scripts/dev-env.sh`, `docs/dev.md`; secrets check fix (PR #2). Commits check accepts GitHub merge commits (PR #3). | Tools found, `.env.local` ignored; dev-env tests 11/11; commit check tests. | Google client in `.env.local` still empty. |
| 1.1 | 18 Nov 2026 | Rust binary (`serve`, `admin`, `backup`, `check`), embedded brotli web app with SPA fallback, `/health` (PR #4). | Served app and `/health` on 127.0.0.1:8791; single 1.6 MB release binary run from an empty folder; 7 HTTP tests. | |
| 1.2 | 18 Nov 2026 | Settings file loading and validation; data folder, `orgs/`, `trash/`, secret file (PR #5). | Missing values listed and start-up stops (binary test); secret created once, mode 600, never printed (binary test). | |
| 1.3 | 18 Nov 2026 | `control.db` and per-company `tazko.db` with migrations, WAL, one writer thread and four readers per file (PR #6). | Migrate fresh and again; a slow write in A does not block B (timed test). | |
| 1.4 | 18 Nov 2026 | Session and company extractors; `/api/v1/me`, `/api/v1/c/{company}` (PR #7). | A member of A gets the same 404 for every company B address and B's file is not opened. | |
| 1.5 | 18 Nov 2026 | Google sign-in with state, nonce, PKCE, ID token checks; invite-only matching; refusal page and audit row; sessions and sign out (PR #8). | Workspace and Gmail accounts sign in (fake Google endpoints); uninvited account refused and audited. | Live sign-in needs the owner's Google client. |
| 1.6 | 18 Nov 2026 | `tazko admin --email`; super admin console API and six screens; trash for 30 days (PR #9). | Every admin action writes an audit row (test); screens compared with the prototype. | |
| 1.7 | 18 Nov 2026 | Company owner API; company home, settings, manage projects shell; switcher (PR #10). | Owner invites, member joins on first sign-in (full flow test); two-company isolation test. | |
| 1.8 | 18 Nov 2026 | Every design token mapped into the Tailwind theme; toasts with Undo; shared empty state and page header (this step). | Side-by-side screenshots of the M1 screens with the prototype, light and dark; token mapping test. | Fonts (open question 1). |
| 1.9 | 18 Nov 2026 | CI: web type check step, web licence check, `cargo deny` (licences, bans, sources) with `deny.toml`; `url` crate removed (PR #12). | `ci.sh` passes on a fresh clone of the branch with no tracked changes afterwards; 29 check tests including licence and product-name cases. | Licence exceptions (open question 4). |
| 1.10 | 18 Nov 2026 | `tazko backup`, `backup restore`, `backup verify`, `tazko check`; `deploy/` setup script, service and timers, sudoers, release script with rollback, bundle script; backup results on the Server page (PR #13). | Restoring a backup into a fresh folder passes `tazko check` (Rust and command-line tests); a broken release rolls itself back (release script test with a fake health check). | On the VM: the rollback and restore checks run for real in 1.11. |


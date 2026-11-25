# Command line

The same program, `tazko`, runs the server and works as a command-line client. Everyone can see these steps under **Settings → Command line**.

## Install

```bash
curl -fsSL https://<server>/cli/install.sh | sh
```

The script checks that this computer matches the system the server was built for (for example `macos-aarch64` or `linux-x86_64`), downloads the program from `https://<server>/cli/download`, and puts it in `~/.local/bin` (or `TAZKO_INSTALL_DIR`). The server only offers its own build; for another system, build it from the repository with `cargo build --release`.

## Sign in

```bash
tazko auth login --host https://<server>
```

1. The program asks the server for a one-time code (`POST /api/v1/cli/device`) and prints it, for example `WDJB-MJHT`.
2. It opens `https://<server>/cli/authorize` in the browser (or prints the address with `--no-browser`). The signed-in person types the code from their terminal; the code is never carried in a link, so nobody can send someone a ready-made approval link. The page then shows the computer name the program gave (marked as what it says it is), what the program will be able to do, and a warning to continue only for a sign-in they started themselves; they press **Authorize** or **Cancel**. A token cannot approve a sign-in; it takes a browser session.
3. The program asks every 5 seconds (`POST /api/v1/cli/token`). After approval it receives a personal token once: scope "read and write cards", valid 90 days, named "Command line on <computer>", listed under Settings → Access tokens where it can be revoked. Codes expire after 10 minutes and can be used once.
4. The token is saved in `~/.config/tazko/token` (or `$XDG_CONFIG_HOME/tazko`, `%APPDATA%\tazko` on Windows, or `TAZKO_CONFIG_DIR`). The folder is readable only by you (0700) and the file too (0600). The token is never printed, except by `tazko auth token`, which exists for scripts. The server address goes in `config.json` next to it.

`TAZKO_TOKEN` and `TAZKO_HOST` override the stored token and server, for scripts and CI. The stored token is only ever sent to the server it was issued by: pointing a command at another server with `--host`, `TAZKO_HOST` or `tazko config set host` asks you to sign in there instead. The server a token belongs to is saved next to it in `config.json` by `tazko auth login` and cleared by `tazko auth logout`. The server address must be `https://`; plain `http://` is only allowed for `localhost` and `127.0.0.1`.

`tazko auth logout` deletes the stored token on this computer. Revoke the token under Settings, Access tokens to stop it working everywhere.

## Commands

| Command | Subcommands |
| --- | --- |
| `tazko auth` | login, logout, status, token |
| `tazko project` | list, members |
| `tazko board` | list, view (`--web` opens it in the browser) |
| `tazko card` | list, view, create, move, done, comment, archive |
| `tazko my` | cards |
| `tazko search "query"` | same operators as the app |
| `tazko api METHOD /path` | any API endpoint, with `--data '{...}'` for a body |
| `tazko config` | get, set host, set company |

- Cards are written `BOARD/CARD`, for example `3/142`. Projects, boards, lists and labels can be given by id or by name.
- `tazko card create --label Bug --label Frontend` adds labels from the board; an unknown label stops before the card is made.
- `tazko my cards --due overdue|today|week|none` keeps open cards that are late, due in the next 24 hours, due in the next 7 days, or have no due date. The server does the filtering (`GET /api/v1/me/cards?due=…`).
- A token limited to some projects sees only its own projects' cards in `tazko my cards`.
- `--company <slug>` picks the company; it defaults to `tazko config set company`, or your only company.
- `--json` prints the JSON answer, `--json id,title` only those fields, and `--jq .cards[].title` parts of it. The `--jq` filter understands paths: `.key`, `.[n]`, `.[]` and their combinations.
- Exit codes: 0 success, 1 failure, 2 wrong usage, 4 not signed in.
- Starting a sign-in is limited to 10 a minute for each address, as the proxy in front reports it (`CF-Connecting-IP` behind the tunnel by default, or the header named in `TAZKO_CLIENT_IP_HEADER`, such as `X-Forwarded-For`), and 120 a minute for the whole server; at most 1000 codes wait at once. Set the header to one the proxy itself writes, so visitors cannot choose their own address.

## Not yet

Shell completion, `tazko update`, the token, webhook and checklist commands, and a system-keychain store are not built yet. The REST API reference is at `/api/docs`.

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
2. It opens `https://<server>/cli/authorize?code=WDJB-MJHT` in the browser (or prints the address with `--no-browser`). The signed-in person checks that the code matches, sees which computer is asking and what the program will be able to do, and presses **Authorize** or **Cancel**. A token cannot approve a sign-in; it takes a browser session.
3. The program asks every 5 seconds (`POST /api/v1/cli/token`). After approval it receives a personal token once: scope "read and write cards", valid 90 days, named "Command line on <computer>", listed under Settings → Access tokens where it can be revoked. Codes expire after 10 minutes and can be used once.
4. The token is saved in `~/.config/tazko/token` (or `$XDG_CONFIG_HOME/tazko`, `%APPDATA%\tazko` on Windows, or `TAZKO_CONFIG_DIR`). The folder is readable only by you (0700) and the file too (0600). The token is never printed, except by `tazko auth token`, which exists for scripts. The server address goes in `config.json` next to it.

`TAZKO_TOKEN` and `TAZKO_HOST` override the stored token and server, for scripts and CI. The server address must be `https://`; plain `http://` is only allowed for `localhost` and `127.0.0.1`.

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

- Cards are written `BOARD/CARD`, for example `3/142`. Boards and lists can be given by id or by name.
- `--company <slug>` picks the company; it defaults to `tazko config set company`, or your only company.
- `--json` prints the JSON answer, `--json id,title` only those fields, and `--jq .cards[].title` parts of it. The `--jq` filter understands paths: `.key`, `.[n]`, `.[]` and their combinations.
- Exit codes: 0 success, 1 failure, 2 wrong usage, 4 not signed in.

## Not yet

Shell completion, `tazko update`, the token, webhook and checklist commands, and a system-keychain store are not built yet. The REST API reference is at `/api/docs`.

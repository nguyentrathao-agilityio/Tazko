# Email

Email is optional. Without it everything works inside the app and invitations are shared by hand. With it, every invite form gets an **Email the invitation** box, and the admin Server page shows the status and can send a test email.

## Settings

Put these in the server settings file (`/etc/tazko/tazko.env` on the server, `.env.local` on a dev machine) and restart Tazko:

| Setting | Needed | Example | Notes |
| --- | --- | --- | --- |
| `TAZKO_SMTP_HOST` | yes | `smtp.gmail.com` | The mail server |
| `TAZKO_SMTP_PORT` | no | `587` | `587` (STARTTLS, the default) or `465` (TLS from the start) |
| `TAZKO_SMTP_USER` | if the server needs a sign-in | `no-reply@hugepigstudio.com` | |
| `TAZKO_SMTP_PASSWORD` | if the server needs a sign-in | an app password | Never shown on any page or in logs |
| `TAZKO_MAIL_FROM` | yes | `Tazko <no-reply@hugepigstudio.com>` | The one address every email comes from |

For a Google Workspace account: turn on 2-Step Verification for the sending account, create an app password, and use `smtp.gmail.com`, port `587`, the account address as user and the app password as password. The From address must be that account or one of its verified aliases.

Tazko refuses to start and names the problem if `TAZKO_SMTP_HOST` or `TAZKO_MAIL_FROM` is missing while any email setting is set, or if `TAZKO_MAIL_FROM` is not a valid address.

## How sending works

- Emails go into a queue in `control.db`. A background worker sends them every few seconds over SMTP.
- The connection is encrypted: port 465 uses TLS from the start; any other port must offer STARTTLS, and Tazko refuses to send (and never signs in) over an unencrypted connection, except to a mail server on the same machine (`127.0.0.1`, `::1`, `localhost`). Certificates are checked against the system's trusted roots.
- A temporary failure (no connection, a 4xx answer) is retried after 1 minute, 5 minutes, 30 minutes, 2 hours and 6 hours, then marked failed. A permanent failure (a 5xx answer such as an unknown recipient) is marked failed at once. The Server page shows the counts for the last 24 hours and the last problem.
- Sent and failed emails are removed from the queue after 30 days.

## What is sent

| Email | When | Limits |
| --- | --- | --- |
| Invitation | Someone ticks **Email the invitation** on a company invite (company settings or admin) or a project invite (project members or Share on a board) for an address that is not yet a member | 3 per address per day, 50 per company per hour. Over the limit the invite is still made and the screen shows the site address to share instead |
| Test | A super admin clicks **Send test email** on the Server page; it goes to their own address | 5 per hour |

Mention, assignment and due-date emails, with each person's choices and an unsubscribe link, come with notification preferences (step 6.4).

An invitation names who sent it and the company (and project), says which address to sign in with, and links to the site's sign-in page. It contains nothing else from the company. Emails carry no tracking: no images, no pixels, no rewritten links. Names that people type are cleaned before they go into headers, so a name cannot add headers such as `Bcc`.

## Trying it on a dev machine

`tazko dev mail` runs a mail catcher on `127.0.0.1` that saves every email as a `.eml` file instead of sending it:

```bash
./target/debug/tazko dev mail --port 2525 --dir ./mail-outbox
```

Then add to `.env.local` and restart the server:

```
TAZKO_SMTP_HOST=127.0.0.1
TAZKO_SMTP_PORT=2525
TAZKO_MAIL_FROM=Tazko <no-reply@localhost.test>
```

Invite someone with **Email the invitation** ticked; within a few seconds a file appears in `./mail-outbox`. Open it in any mail program to see it as the recipient would.

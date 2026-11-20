# Webhooks

Company owners add webhooks under **Integrations → Webhooks**. Tazko then sends a `POST` with a JSON body to the address each time a chosen event happens.

## Who sees what

- Only company owners can list, add, pause, test or delete webhooks. Members get `403`.
- A webhook sees what the owner who added it can see, and no more. Events come only from projects that owner belongs to (all of them, or the projects chosen when adding it) and from boards that owner can open. Restricted boards the owner is not on never produce a delivery.
- The check runs again for every event, so when the owner leaves a project or loses access to a board, that project or board stops sending.
- If the owner who added it is no longer a company owner, the webhook stops sending. Another owner can delete it and add a new one.
- A new or resumed webhook starts from now: events from before it was added, or from while it was paused, are not sent.

## Events

| Event | When |
| --- | --- |
| `card.created` | A card is added (one delivery per card when several are added at once) |
| `card.updated` | A card's title, description, dates, labels or other details change, or it is marked not done |
| `card.moved` | A card moves to another list |
| `card.completed` | A card is marked done (also when a repeating card rolls over) |
| `card.archived` | A card is archived |
| `comment.created` | Someone comments on a card |
| `member.added` | Someone joins a project |
| `board.created` | A board is made |
| `ping` | **Send test event** was clicked |

## The message

```http
POST /hooks/tazko HTTP/1.1
Content-Type: application/json
User-Agent: Tazko-Webhooks/1
Tazko-Event: card.moved
Tazko-Delivery: 4127
Tazko-Signature: t=1763600000,v1=5f0c…e2

{
  "event": "card.moved",
  "company": "tazko-demo",
  "created_at": 1763599998,
  "actor": { "id": 3, "name": "Mai Tran", "email": "mai@example.com" },
  "project": { "id": 1, "name": "Website relaunch" },
  "board": { "id": 2, "name": "Sprint 12" },
  "card": {
    "id": 41,
    "title": "Ship the beta",
    "description": "Plain text of the description",
    "due_at": 1763856000,
    "done": false,
    "archived": false,
    "list": { "id": 7, "name": "Doing" },
    "created_at": 1763000000,
    "updated_at": 1763599998
  },
  "from_list": { "id": 6, "name": "To do" }
}
```

Extra fields by event: `card.updated` has `changed` (the names of the fields that changed), `card.moved` has `from_list`, `comment.created` has `comment` (`id`, `body`, `created_at`), `member.added` has `member` and `role`. Times are Unix seconds.

Tazko waits 10 seconds for an answer and does not follow redirects. Any `2xx` answer counts as delivered.

## Checking the signature

When a webhook is added, Tazko shows its signing secret (`whsec_…`) once. Keep it in your receiver's settings. If it is lost, delete the webhook and add it again.

`Tazko-Signature` is `t=<unix seconds>,v1=<hex>`, where `<hex>` is HMAC-SHA256 with the secret as the key over `<t>.<raw body>`. To check a message:

1. Split the header into `t` and `v1`.
2. Compute HMAC-SHA256(secret, `t + "." + raw request body`) as lowercase hex. Use the raw bytes you received, before parsing the JSON.
3. Compare it with `v1` using a constant-time comparison.
4. Refuse messages whose `t` is more than 5 minutes away from your clock, so an old message cannot be replayed.
5. `Tazko-Delivery` stays the same when a delivery is retried, so it can be used to ignore duplicates. **Redeliver** creates a new delivery number.

Node.js:

```js
import crypto from 'node:crypto'

export function verify(secret, header, rawBody) {
  const { t, v1 } = Object.fromEntries(header.split(',').map((p) => p.split('=')))
  if (Math.abs(Date.now() / 1000 - Number(t)) > 300) return false
  const expected = crypto.createHmac('sha256', secret).update(`${t}.${rawBody}`).digest('hex')
  return v1.length === expected.length && crypto.timingSafeEqual(Buffer.from(v1), Buffer.from(expected))
}
```

Python:

```python
import hashlib, hmac, time

def verify(secret: str, header: str, raw_body: bytes) -> bool:
    parts = dict(p.split("=", 1) for p in header.split(","))
    if abs(time.time() - int(parts["t"])) > 300:
        return False
    signed = parts["t"].encode() + b"." + raw_body
    expected = hmac.new(secret.encode(), signed, hashlib.sha256).hexdigest()
    return hmac.compare_digest(expected, parts["v1"])
```

## Retries and the log

A delivery that gets no `2xx` answer is tried again after 1 minute, 5 minutes, 30 minutes, 2 hours, 6 hours and 12 hours (seven tries over about a day), then marked failed. **Deliveries** shows the latest 50 with the answer code, time taken and when the next try is. **Redeliver** sends a failed delivery again. A paused webhook keeps its queued deliveries until it is resumed.

## Addresses

Addresses must start with `https://` and must reach the public internet. Tazko refuses addresses that resolve to private, loopback, link-local or other internal ranges (for example `10.x`, `192.168.x`, `127.0.0.1`, `169.254.169.254`, `::1`, `fd00::/8`), addresses with a user name or password, and other schemes. The address is checked when it is saved and again at every delivery, and the connection only goes to the addresses that passed the check, so a name that later points somewhere internal is refused.

For local development only, `TAZKO_WEBHOOKS_ALLOW_LOCAL=1` lets webhooks use `http://` and loopback addresses (`127.0.0.1`, `::1`, `localhost`). Private network ranges stay refused. Do not set it on a public server.

## How the secret is kept

Tazko stores a random salt per webhook, not the secret. The secret is derived from the server secret (`TAZKO_SECRET`, or the `secret` file in the data folder) and that salt each time a message is signed, so a copy of a company database alone does not reveal it. Changing the server secret changes every webhook's signing secret; receivers then need the webhooks added again.

# Load test

How fast one Tazko server answers when many people use one company's boards at once. Run on a local release build with throwaway data, never against the demo, the real data or anyone else's server.

## How to run

```bash
scripts/load-test.sh
```

It builds the release binary and `examples/load.rs`, makes a throwaway data folder, adds a super admin and the demo company (`tazko dev seed`), writes a sign-in session into the throwaway `control.db`, starts the server on `127.0.0.1:8796`, adds 500 more cards, and runs the load. Everything is deleted at the end. It needs `sqlite3`, `openssl` and `curl`, and refuses to start if something already answers on the port.

Settings, all optional: `USERS` (parallel users, 20), `DURATION` (seconds, 30), `CARDS` (extra cards, 500), `PORT` (8796).

Each user runs requests back to back with no pause, as one signed-in person:

| Request | Share | What |
| --- | ---: | --- |
| board | 55% | `GET /boards/{id}`, the whole board with every list and card |
| card | 20% | `GET /boards/{id}/cards/{card}`, the card dialog |
| search | 10% | `GET /search?q=design` |
| comment | 10% | `POST …/cards/{card}/comments` |
| move | 5% | `POST …/cards/{card}/move` to another list |

A request counts as an error when it gets no answer or an answer other than 2xx. Times are measured by the client, from sending the request to having read the whole answer.

## Results, 26 Nov 2026

Machine: Apple M1, 8 cores, 16 GB, macOS; the load generator ran on the same machine and shared its CPU. Data: the demo company with 2 boards and 520 cards (about 260 per board). Release build of `dev` at `76faf5e`.

### 20 users, 30 seconds

| Request | Count | Errors | p50 ms | p95 ms | p99 ms | Max ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| all | 35,884 | 0 | 15.7 | 32.5 | 47.1 | 234.7 |
| board | 19,736 | 0 | 18.8 | 35.5 | 51.5 | 234.7 |
| card | 7,177 | 0 | 13.5 | 28.9 | 40.5 | 163.4 |
| comment | 3,588 | 0 | 9.2 | 22.7 | 33.7 | 169.3 |
| move | 1,794 | 0 | 8.7 | 21.7 | 32.6 | 100.5 |
| search | 3,589 | 0 | 10.9 | 24.1 | 36.7 | 153.4 |

1,196 requests a second. Server memory 10 MB before, 40 MB after.

### 50 users, 30 seconds

| Request | Count | Errors | p50 ms | p95 ms | p99 ms | Max ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| all | 39,443 | 0 | 36.6 | 72.6 | 94.4 | 170.7 |
| board | 21,694 | 0 | 41.9 | 76.9 | 98.5 | 170.7 |
| card | 7,889 | 0 | 36.7 | 72.8 | 92.0 | 135.8 |
| comment | 3,944 | 0 | 24.1 | 54.8 | 75.4 | 128.4 |
| move | 1,972 | 0 | 23.5 | 51.7 | 72.6 | 129.6 |
| search | 3,944 | 0 | 26.8 | 55.5 | 74.5 | 119.5 |

1,313 requests a second. Server memory 10 MB before, 42 MB after.

## What it means

- No errors in about 75,000 requests, including about 11,000 writes to one company file while it was being read.
- Throughput levels off near 1,200 to 1,300 requests a second on this machine: going from 20 to 50 users adds little throughput and doubles the response times, so the extra users wait in line. Part of that limit is the load generator sharing the CPU.
- Writes (comment, move) are faster than reading a whole board: a full board of about 260 cards is the most expensive request. With the plain demo data (about 20 cards a board) every request takes under 4 ms at p99 and the server answers about 4,000 requests a second, so the board size, not the number of users, sets the cost. A real person makes a request every few seconds, not every 15 ms, so 50 users here stand for far more real people.
- Memory stays small (about 40 MB) under load.

Not covered: live connections (WebSocket fan-out), uploads, several companies at once, and the time to first load the web app.

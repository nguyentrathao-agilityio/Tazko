#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

PORT=${PORT:-8796}
USERS=${USERS:-20}
DURATION=${DURATION:-30}
CARDS=${CARDS:-500}
BASE="http://127.0.0.1:$PORT"

for tool in sqlite3 openssl curl; do
  command -v "$tool" >/dev/null || { echo "load-test: $tool is needed" >&2; exit 2; }
done
if curl -sS --max-time 2 -o /dev/null "$BASE/" 2>/dev/null; then
  echo "load-test: something already answers on port $PORT; choose another with PORT=" >&2
  exit 1
fi

cargo build --release --locked --bins --examples
bin=target/release/tazko

tmp=$(mktemp -d)
server=
cleanup() {
  if [ -n "$server" ]; then
    kill "$server" 2>/dev/null || true
    wait "$server" 2>/dev/null || true
  fi
  rm -rf "$tmp"
}
trap cleanup EXIT

fake=$(openssl rand -hex 16)
cat >"$tmp/env" <<ENV
TAZKO_HOST=127.0.0.1
TAZKO_PORT=$PORT
TAZKO_PUBLIC_URL=$BASE
TAZKO_DATA_DIR=$tmp/data
TAZKO_GOOGLE_CLIENT_ID=load-test.example
TAZKO_GOOGLE_CLIENT_SECRET=$fake
ENV

"$bin" admin --env-file "$tmp/env" --email load-admin@example.com >/dev/null
"$bin" dev seed --env-file "$tmp/env" --owner load-admin@example.com >/dev/null

session=$(openssl rand -hex 32)
csrf=$(openssl rand -hex 32)
hash=$(printf %s "$session" | openssl dgst -sha256 -r | cut -d' ' -f1)
sqlite3 "$tmp/data/control.db" "INSERT INTO sessions (id_hash, user_id, csrf_token, user_agent, created_at, last_used_at, expires_at)
  SELECT '$hash', id, '$csrf', 'load test', unixepoch(), unixepoch(), unixepoch() + 30 * 86400 FROM users WHERE email = 'load-admin@example.com'"

"$bin" serve --env-file "$tmp/env" >"$tmp/serve.log" 2>&1 &
server=$!
for _ in $(seq 50); do
  curl -fsS --max-time 1 "$BASE/health" >/dev/null 2>&1 && break
  if ! kill -0 "$server" 2>/dev/null; then
    break
  fi
  sleep 0.2
done
if ! curl -fsS --max-time 2 "$BASE/health" >/dev/null 2>&1; then
  echo "load-test: the server did not start; its log:" >&2
  cat "$tmp/serve.log" >&2
  exit 1
fi

rss() { ps -o rss= -p "$server" | tr -d ' '; }
echo "server memory before: $(( $(rss) / 1024 )) MB"
LOAD_SESSION=$session LOAD_CSRF=$csrf target/release/examples/load \
  --url "$BASE" --users "$USERS" --seconds "$DURATION" --cards "$CARDS"
echo "server memory after: $(( $(rss) / 1024 )) MB"
echo "data folder: $(du -sh "$tmp/data" | cut -f1)"

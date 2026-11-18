#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
pass=0; fail=0

expect() {
  local want=$1 name=$2; shift 2
  if "$@" >/dev/null 2>&1; then got=pass; else got=fail; fi
  if [ "$got" = "$want" ]; then pass=$((pass+1)); echo "  ok   $name"
  else fail=$((fail+1)); echo "  BAD  $name (wanted $want, got $got)"; fi
}

fake_bin() {
  local d="$TMP/bin-$1"; mkdir -p "$d"
  for t in cargo rustc rustfmt cargo-clippy cargo-deny npm pnpm sqlite3; do
    printf '#!/bin/sh\necho "%s 1.0.0"\n' "$t" > "$d/$t"
  done
  printf '#!/bin/sh\necho "%s"\n' "$2" > "$d/node"
  chmod +x "$d"/*
  echo "$d"
}

new_repo() {
  local d="$TMP/$1"; mkdir -p "$d/scripts"; cd "$d"
  git init -q -b main
  cp "$ROOT/scripts/dev-env.sh" scripts/
  cp "$ROOT/.env.example" .
  cp "$ROOT/.gitignore" .
}

run_check() { PATH="$1:/usr/bin:/bin" bash scripts/dev-env.sh check; }

good=$(fake_bin good 24)

new_repo ok
expect pass "all tools present and .env.local ignored" run_check "$good"

new_repo unignored; printf '/target/\n' > .gitignore
expect fail ".env.local not ignored" run_check "$good"

new_repo tracked; echo "X=1" > .env.local; git add -f .env.local
expect fail ".env.local tracked" run_check "$good"

new_repo missing; nopnpm=$(fake_bin nopnpm 24); rm "$nopnpm/pnpm"
expect fail "pnpm missing" run_check "$nopnpm"

new_repo nocargo; nocargo=$(fake_bin nocargo 24); rm "$nocargo/cargo"
expect fail "cargo missing" run_check "$nocargo"

new_repo oldnode; old=$(fake_bin old 20)
expect fail "node 20 is below the minimum" run_check "$old"

new_repo oddnode; odd=$(fake_bin odd 25)
expect fail "node 25 is not an LTS line" run_check "$odd"

new_repo init
expect pass "init creates .env.local" bash scripts/dev-env.sh init
expect pass "init copies the example" cmp .env.example .env.local
expect pass "init makes .env.local owner-only" test "$(stat -f %Lp .env.local 2>/dev/null || stat -c %a .env.local)" = 600
echo "TAZKO_PORT=9999" > .env.local
bash scripts/dev-env.sh init >/dev/null
expect pass "init leaves an existing .env.local alone" grep -q 9999 .env.local

echo "  $pass passed, $fail failed"
[ "$fail" -eq 0 ]

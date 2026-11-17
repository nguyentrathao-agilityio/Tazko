#!/usr/bin/env bash
# Tests for scripts/checks.py. Each case builds a throw-away repository in a
# temporary folder and expects the check to pass or fail.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
CHECKS="$ROOT/scripts/checks.py"
PY=${PYTHON:-python3}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
pass=0; fail=0

new_repo() {
  local d="$TMP/$1"; mkdir -p "$d"; cd "$d"
  git init -q -b main
  git config user.name "Owner Person"; git config user.email "owner@example.com"
  git config commit.gpgsign false
}
expect() { # expect pass|fail "name" cmd...
  local want=$1 name=$2; shift 2
  if "$@" >/dev/null 2>&1; then got=pass; else got=fail; fi
  if [ "$got" = "$want" ]; then pass=$((pass+1)); echo "  ok   $name"
  else fail=$((fail+1)); echo "  BAD  $name (wanted $want, got $got)"; fi
}
commit() { echo "$RANDOM" >> f.txt; git add f.txt; git commit -q -F - "$@"; }

# Words are assembled at run time so this file does not contain them.
tool_word=$(printf '%s' 'Cla' 'ude')
product_word=$(printf '%s' 'Tre' 'llo')

# commits: one clean commit by the owner passes
new_repo clean; printf 'Seed repository\n\nAdd plan and design.\n' | commit
expect pass "clean owner commit" "$PY" "$CHECKS" commits

new_repo coauthor
printf 'Seed\n\nCo-Authored-By: %s <bot@example.com>\n' "$tool_word" | commit
expect fail "co-author trailer" "$PY" "$CHECKS" commits

new_repo generated; printf 'Seed\n\nGenerated with some tool\n' | commit
expect fail "generated-with line" "$PY" "$CHECKS" commits

new_repo session; printf 'Seed\n\nTool-Session: https://example.com/s/1\n' | commit
expect fail "session trailer" "$PY" "$CHECKS" commits

new_repo toolname; printf 'Seed with help from %s\n' "$tool_word" | commit
expect fail "tool named in message" "$PY" "$CHECKS" commits

new_repo author; printf 'Seed\n' | commit --author "Someone Else <else@example.com>"
expect fail "author is not the owner" "$PY" "$CHECKS" commits

new_repo empty
expect pass "no commits yet" "$PY" "$CHECKS" commits

new_repo msg; printf 'Remove generated files\n' > m.txt
expect pass "ordinary use of the word generated" "$PY" "$CHECKS" commit-msg m.txt
printf 'Seed\n\nCo-authored-by: x <x@example.com>\n' > m2.txt
expect fail "commit-msg catches co-author" "$PY" "$CHECKS" commit-msg m2.txt

# names
new_repo names; echo "Import from a board export file." > a.md; git add a.md
expect pass "no product named" "$PY" "$CHECKS" names
echo "Works like $product_word." > b.md; git add b.md
expect fail "product named in a file" "$PY" "$CHECKS" names

# secrets
new_repo secrets; echo "TAZKO_PORT=8791" > notes.md; git add notes.md
expect pass "no secrets" "$PY" "$CHECKS" secrets
printf 'TAZKO_GOOGLE_CLIENT_SECRET=%s\n' "$(printf 'GOCSPX-%s' abcdefghijklmnopqrstuvwx)" > conf.txt; git add conf.txt
expect fail "client secret in a file" "$PY" "$CHECKS" secrets
new_repo envfile; echo "X=1" > tazko.env; git add -f tazko.env
expect fail "settings file tracked" "$PY" "$CHECKS" secrets

# js budget
new_repo js; mkdir -p dist
expect pass "empty dist under budget" "$PY" "$CHECKS" js-budget dist
"$PY" -c "import os;open('dist/a.js','wb').write(os.urandom(260*1024))"
expect fail "260 KB of incompressible JS is over budget" "$PY" "$CHECKS" js-budget dist

echo "  $pass passed, $fail failed"
[ "$fail" -eq 0 ]

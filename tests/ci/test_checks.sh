#!/usr/bin/env bash
# Tests for scripts/checks.py. Each case builds a throw-away repository in a
# temporary folder and expects the check to pass or fail.
set -euo pipefail
unset TAZKO_OWNER_NAME TAZKO_OWNER_EMAIL
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

new_repo productname; printf 'Import boards from %s\n' "$product_word" | commit
expect fail "product named in message" "$PY" "$CHECKS" commits

new_repo author; printf 'Seed\n' | commit --author "Someone Else <else@example.com>"
expect fail "author is not the owner" "$PY" "$CHECKS" commits

gh_merge() { # gh_merge "message": merge branch topic into main as the hosting service does
  GIT_AUTHOR_NAME="Owner Web" GIT_AUTHOR_EMAIL="123+owner@users.noreply.github.com" \
  GIT_COMMITTER_NAME="GitHub" GIT_COMMITTER_EMAIL="noreply@github.com" \
  git merge -q --no-ff topic -m "$1"
}
new_repo ghmerge; printf 'Seed\n' | commit; git switch -q -c topic; printf 'Add step\n' | commit
git switch -q main; gh_merge "Merge pull request #1 from owner/topic"
expect pass "merge commit made by GitHub" "$PY" "$CHECKS" commits

new_repo ghmergemsg; printf 'Seed\n' | commit; git switch -q -c topic; printf 'Add step\n' | commit
git switch -q main; gh_merge "$(printf 'Merge pull request #1\n\nCo-Authored-By: %s <bot@example.com>' "$tool_word")"
expect fail "GitHub merge commit with attribution" "$PY" "$CHECKS" commits

new_repo ghsquash; printf 'Seed\n' | commit
printf 'Squashed step\n' | GIT_COMMITTER_NAME="GitHub" GIT_COMMITTER_EMAIL="noreply@github.com" commit
expect fail "non-merge commit by GitHub" "$PY" "$CHECKS" commits

new_repo othermerge; printf 'Seed\n' | commit; git switch -q -c topic; printf 'Add step\n' | commit
git switch -q main
GIT_COMMITTER_NAME="Someone Else" GIT_COMMITTER_EMAIL="else@example.com" git merge -q --no-ff topic -m "Merge topic"
expect fail "merge commit by someone else" "$PY" "$CHECKS" commits

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
new_repo placeholders; printf 'TAZKO_SECRET=\nTAZKO_MAX_UPLOAD_MB=10\nTAZKO_GOOGLE_CLIENT_SECRET=\nTAZKO_SMTP_PASSWORD=\nTAZKO_MAIL_FROM=x\n' > env.example; git add env.example
expect pass "empty secret placeholders" "$PY" "$CHECKS" secrets
printf 'TAZKO_SECRET=%s\n' "$(printf 'x%.0s' $(seq 32))" > env2.example; git add env2.example
expect fail "filled TAZKO_SECRET" "$PY" "$CHECKS" secrets
new_repo jslic
printf '{"MIT":[{"name":"react"}],"Apache-2.0":[{"name":"cva"}],"(MIT OR CC0-1.0)":[{"name":"dual"}]}' > prod.json
printf '{"MIT":[{"name":"react"},{"name":"vite"}],"MPL-2.0":[{"name":"lightningcss"},{"name":"lightningcss-linux-x64-gnu"}],"BlueOak-1.0.0":[{"name":"lru-cache"}]}' > all.json
expect pass "allowed and listed build-tool licences" "$PY" "$CHECKS" js-licenses prod.json all.json
printf '{"MIT":[{"name":"react"}],"GPL-3.0":[{"name":"copyleft"}]}' > prod2.json
expect fail "a shipped GPL package" "$PY" "$CHECKS" js-licenses prod2.json all.json
printf '{"MPL-2.0":[{"name":"lightningcss"}]}' > prod3.json
expect fail "a build-tool exception that ships" "$PY" "$CHECKS" js-licenses prod3.json all.json
printf '{"MIT":[{"name":"vite"}],"MPL-2.0":[{"name":"new-tool"}]}' > all2.json
expect fail "an unlisted build tool" "$PY" "$CHECKS" js-licenses prod.json all2.json
printf '{"MIT":[{"name":"vite"}],"GPL-3.0":[{"name":"lightningcss"}]}' > all3.json
expect fail "a listed tool under another licence" "$PY" "$CHECKS" js-licenses prod.json all3.json
printf '{"MIT":[{"name":"react"}],"0BSD":[{"name":"tslib"}]}' > prod5.json
expect pass "a listed dependency that is never bundled" "$PY" "$CHECKS" js-licenses prod5.json all.json
printf '{"MIT":[{"name":"react"}],"0BSD":[{"name":"other"}]}' > prod6.json
expect fail "an unlisted 0BSD package" "$PY" "$CHECKS" js-licenses prod6.json all.json
printf '{"MIT AND GPL-3.0":[{"name":"mixed"}]}' > prod4.json
expect fail "an AND with a disallowed licence" "$PY" "$CHECKS" js-licenses prod4.json all.json
new_repo envfile; echo "X=1" > tazko.env; git add -f tazko.env
expect fail "settings file tracked" "$PY" "$CHECKS" secrets

# js budget
new_repo js; mkdir -p dist
expect pass "empty dist under budget" "$PY" "$CHECKS" js-budget dist
"$PY" -c "import os;open('dist/a.js','wb').write(os.urandom(260*1024))"
expect fail "260 KB of incompressible JS is over budget" "$PY" "$CHECKS" js-budget dist

echo "  $pass passed, $fail failed"
[ "$fail" -eq 0 ]

#!/usr/bin/env bash
# Tests for scripts/checks.py. Each case builds a throw-away repository in a
# temporary folder and expects the check to pass or fail.
set -euo pipefail
export TAZKO_OWNER_NAME="Owner Person" TAZKO_OWNER_EMAIL="owner@example.com"
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
checks() { "$PY" "$CHECKS" "$@"; }
without_owner_env() { (unset TAZKO_OWNER_NAME TAZKO_OWNER_EMAIL; "$@"); }
edit_settings() { "$PY" -c 'import json,sys; p=".claude/settings.json"; d=json.load(open(p)); exec(sys.argv[1]); json.dump(d, open(p, "w"))' "$1"; }

# Words are assembled at run time so this file does not contain them.
tool_word=$(printf '%s' 'Cla' 'ude')
product_word=$(printf '%s' 'Tre' 'llo')

# commits: one clean commit by the owner passes
new_repo clean; printf 'Seed repository\n\nAdd plan and design.\n' | commit
expect pass "clean owner commit" checks commits

new_repo coauthor
printf 'Seed\n\nCo-Authored-By: %s <bot@example.com>\n' "$tool_word" | commit
expect fail "co-author trailer" checks commits

new_repo generated; printf 'Seed\n\nGenerated with some tool\n' | commit
expect fail "generated-with line" checks commits

new_repo session; printf 'Seed\n\nTool-Session: https://example.com/s/1\n' | commit
expect fail "session trailer" checks commits

new_repo toolname; printf 'Seed with help from %s\n' "$tool_word" | commit
expect fail "tool named in message" checks commits

new_repo productname; printf 'Import boards from %s\n' "$product_word" | commit
expect fail "product named in message" checks commits

new_repo author; printf 'Seed\n' | commit --author "Someone Else <else@example.com>"
expect fail "author is not the owner" checks commits

gh_merge() { # gh_merge "message": merge branch topic into main as the hosting service does
  GIT_AUTHOR_NAME="Owner Web" GIT_AUTHOR_EMAIL="123+owner@users.noreply.github.com" \
  GIT_COMMITTER_NAME="GitHub" GIT_COMMITTER_EMAIL="noreply@github.com" \
  git merge -q --no-ff topic -m "$1"
}
new_repo ghmerge; printf 'Seed\n' | commit; git switch -q -c topic; printf 'Add step\n' | commit
git switch -q main; gh_merge "Merge pull request #1 from owner/topic"
expect pass "merge commit made by GitHub" checks commits

new_repo ghmergemsg; printf 'Seed\n' | commit; git switch -q -c topic; printf 'Add step\n' | commit
git switch -q main; gh_merge "$(printf 'Merge pull request #1\n\nCo-Authored-By: %s <bot@example.com>' "$tool_word")"
expect fail "GitHub merge commit with attribution" checks commits

new_repo ghsquash; printf 'Seed\n' | commit
printf 'Squashed step\n' | GIT_COMMITTER_NAME="GitHub" GIT_COMMITTER_EMAIL="noreply@github.com" commit
expect fail "non-merge commit by GitHub" checks commits

new_repo othermerge; printf 'Seed\n' | commit; git switch -q -c topic; printf 'Add step\n' | commit
git switch -q main
GIT_COMMITTER_NAME="Someone Else" GIT_COMMITTER_EMAIL="else@example.com" git merge -q --no-ff topic -m "Merge topic"
expect fail "merge commit by someone else" checks commits

new_repo empty
expect pass "no commits yet" checks commits

new_repo msg; printf 'Remove generated files\n' > m.txt
expect pass "ordinary use of the word generated" checks commit-msg m.txt
printf 'Seed\n\nCo-authored-by: x <x@example.com>\n' > m2.txt
expect fail "commit-msg catches co-author" checks commit-msg m2.txt

# names
new_repo names; echo "Import from a board export file." > a.md; git add a.md
expect pass "no product named" checks names
echo "Works like $product_word." > b.md; git add b.md
expect fail "product named in a file" checks names

# secrets
new_repo secrets; echo "TAZKO_PORT=8791" > notes.md; git add notes.md
expect pass "no secrets" checks secrets
printf 'TAZKO_GOOGLE_CLIENT_SECRET=%s\n' "$(printf 'GOCSPX-%s' abcdefghijklmnopqrstuvwx)" > conf.txt; git add conf.txt
expect fail "client secret in a file" checks secrets
new_repo placeholders; printf 'TAZKO_SECRET=\nTAZKO_MAX_UPLOAD_MB=10\nTAZKO_GOOGLE_CLIENT_SECRET=\nTAZKO_SMTP_PASSWORD=\nTAZKO_MAIL_FROM=x\n' > env.example; git add env.example
expect pass "empty secret placeholders" checks secrets
printf 'TAZKO_SECRET=%s\n' "$(printf 'x%.0s' $(seq 32))" > env2.example; git add env2.example
expect fail "filled TAZKO_SECRET" checks secrets
new_repo duckdns; printf 'DUCKDNS_TOKEN=0a1b2c3d-1111-2222-3333-444455556666\n' > duckdns.env.example; git add duckdns.env.example
expect fail "filled DuckDNS token" checks secrets
new_repo duckdnsurl; printf 'curl "https://www.duckdns.org/update?domains=x&token=%s"\n' "0a1b2c3d-1111-2222-3333-444455556666" > notes.md; git add notes.md
expect fail "DuckDNS token in an update link" checks secrets
new_repo jslic
printf '{"MIT":[{"name":"react"}],"Apache-2.0":[{"name":"cva"}],"(MIT OR CC0-1.0)":[{"name":"dual"}]}' > prod.json
printf '{"MIT":[{"name":"react"},{"name":"vite"}],"MPL-2.0":[{"name":"lightningcss"},{"name":"lightningcss-linux-x64-gnu"}],"BlueOak-1.0.0":[{"name":"lru-cache"}]}' > all.json
expect pass "allowed and listed build-tool licences" checks js-licenses prod.json all.json
printf '{"MIT":[{"name":"react"}],"GPL-3.0":[{"name":"copyleft"}]}' > prod2.json
expect fail "a shipped GPL package" checks js-licenses prod2.json all.json
printf '{"MPL-2.0":[{"name":"lightningcss"}]}' > prod3.json
expect fail "a build-tool exception that ships" checks js-licenses prod3.json all.json
printf '{"MIT":[{"name":"vite"}],"MPL-2.0":[{"name":"new-tool"}]}' > all2.json
expect fail "an unlisted build tool" checks js-licenses prod.json all2.json
printf '{"MIT":[{"name":"vite"}],"GPL-3.0":[{"name":"lightningcss"}]}' > all3.json
expect fail "a listed tool under another licence" checks js-licenses prod.json all3.json
printf '{"MIT":[{"name":"react"}],"0BSD":[{"name":"tslib"}]}' > prod5.json
expect pass "a listed dependency that is never bundled" checks js-licenses prod5.json all.json
printf '{"MIT":[{"name":"react"}],"0BSD":[{"name":"other"}]}' > prod6.json
expect fail "an unlisted 0BSD package" checks js-licenses prod6.json all.json
printf '{"MIT AND GPL-3.0":[{"name":"mixed"}]}' > prod4.json
expect fail "an AND with a disallowed licence" checks js-licenses prod4.json all.json
printf '{"(MIT OR Apache-2.0) AND BSD-3-Clause":[{"name":"grouped"}]}' > prod7.json
expect pass "a grouped OR inside an AND" checks js-licenses prod7.json all.json
printf '{"(MIT) AND (GPL-3.0)":[{"name":"grouped-bad"}]}' > prod8.json
expect fail "a grouped AND with a disallowed licence" checks js-licenses prod8.json all.json
new_repo envfile; echo "X=1" > tazko.env; git add -f tazko.env
expect fail "settings file tracked" checks secrets

# js budget
new_repo js; mkdir -p dist
expect pass "empty dist under budget" checks js-budget dist
"$PY" -c "import os;open('dist/a.js','wb').write(os.urandom(260*1024))"
expect fail "260 KB of incompressible JS is over budget" checks js-budget dist

new_repo localidentity; printf 'Seed\n' | commit
expect fail "local git identity is not the owner without the variables" without_owner_env checks commits

# comments
new_repo comments; mkdir -p src web/src/a migrations/tenant docs
printf 'fn main() {\n    let u = "https://example.com";\n}\n' > src/main.rs
printf "const r = /^https?:\\/\\//\nconst s = 'a // b'\n" > web/src/a/x.ts
printf '/// <reference types="node" />\nexport {}\n' > web/src/a/t.test.ts
printf '.a { color: red; }\n' > web/src/a/x.css
printf 'CREATE TABLE t (id INTEGER);\n' > migrations/tenant/0001.sql
printf '// notes are fine outside the source folders\n' > docs/n.rs
git add -A
expect pass "no comments in source" checks comments
printf 'fn f() {}\n// why\n' > src/b.rs; git add -A
expect fail "line comment in Rust" checks comments
new_repo comments2; mkdir -p src; printf 'fn f() {} // why\n' > src/b.rs; git add -A
expect fail "trailing comment in Rust" checks comments
new_repo comments3; mkdir -p src; printf '/// docs\nfn f() {}\n' > src/b.rs; git add -A
expect fail "doc comment in Rust" checks comments
new_repo comments4; mkdir -p web/src; printf 'export const A = () => <div>{/* x */}</div>\n' > web/src/a.tsx; git add -A
expect fail "comment in JSX" checks comments
new_repo comments5; mkdir -p web/src; printf '/* x */\n.a {}\n' > web/src/a.css; git add -A
expect fail "comment in CSS" checks comments
new_repo comments6; mkdir -p migrations/control; printf -- '-- x\nCREATE TABLE t (id INTEGER);\n' > migrations/control/0001.sql; git add -A
expect fail "comment in a migration" checks comments

# prototype-only
new_repo proto; mkdir -p web/src; printf 'export const t = "Board"\n' > web/src/a.ts; git add -A
expect pass "no prototype-only controls" checks prototype-only
printf 'export const t = "View as"\n' > web/src/b.ts; git add -A
expect fail "prototype role switch built" checks prototype-only

# web-paths
new_repo webpaths; mkdir -p web/src; printf "export const companyApi = (s: string) => \`/api/v1/c/\${s}\`\n" > web/src/paths.ts
printf "import { companyApi } from '@/paths'\nexport const x = companyApi('a')\n" > web/src/a.ts
printf "const url = '/api/v1/me'\n" > web/src/a.test.ts; git add -A
expect pass "paths only in paths.ts and tests" checks web-paths
printf "export const y = \`/api/v1/c/\${1}/boards\`\n" > web/src/b.ts; git add -A
expect fail "API path built by hand" checks web-paths
rm web/src/b.ts; printf "export const z = (s: string) => \`/c/\${s}/b/1\`\n" > web/src/c.tsx; git add -A
expect fail "route built by hand" checks web-paths
rm web/src/c.tsx; printf "export const w = (b: string) => \`\${b}/c/\${2}\`\n" > web/src/d.js; git add -A
expect fail "card route appended by hand in a JS file" checks web-paths

# design
new_repo design; mkdir -p design scripts; echo a > design/a.html; git add -A
expect fail "no design list yet" checks design
checks design --update >/dev/null
expect pass "design matches its list" checks design
echo b >> design/a.html
expect fail "design file changed" checks design
checks design --update >/dev/null; echo c > design/b.css; git add -A
expect fail "design file added" checks design

# migrations
new_repo migrations; mkdir -p migrations/tenant scripts; echo a > migrations/tenant/0001.sql; git add -A
expect fail "migration not listed yet" checks migrations
checks migrations --update >/dev/null
expect pass "migrations match their list" checks migrations
echo b >> migrations/tenant/0001.sql
expect fail "committed migration changed" checks migrations
expect fail "update refuses a changed migration" checks migrations --update
git checkout -q migrations/tenant/0001.sql; echo c > migrations/tenant/0002.sql; git add -A
expect fail "new migration not listed" checks migrations
checks migrations --update >/dev/null
expect pass "new migration listed" checks migrations
git rm -qf migrations/tenant/0001.sql
expect fail "committed migration removed" checks migrations

# settings
new_repo settings; mkdir -p .claude
cp "$ROOT/.claude/settings.json" .claude/settings.json
expect pass "repository settings" checks settings
edit_settings 'd["permissions"]["ask"].remove("Bash(sudo:*)")'
expect fail "sudo no longer asks" checks settings
cp "$ROOT/.claude/settings.json" .claude/settings.json
edit_settings 'd["includeCoAuthoredBy"] = True'
expect fail "attribution turned back on" checks settings
cp "$ROOT/.claude/settings.json" .claude/settings.json
edit_settings 'd["hooks"]["PreToolUse"][0]["matcher"] = "Edit|Write"'
expect fail "write guard no longer covers shell commands" checks settings
cp "$ROOT/.claude/settings.json" .claude/settings.json
edit_settings 'd.pop("hooks")'
expect fail "write guard removed" checks settings
cp "$ROOT/.claude/settings.json" .claude/settings.json
edit_settings 'd["hooks"]["PreToolUse"][0]["matcher"] = ""'
expect pass "write guard on every tool" checks settings
cp "$ROOT/.claude/settings.json" .claude/settings.json
edit_settings 'd["hooks"]["PreToolUse"][0]["matcher"] = "Edit|("'
expect fail "broken matcher is a failure, not a crash" checks settings

echo "  $pass passed, $fail failed"
[ "$fail" -eq 0 ]

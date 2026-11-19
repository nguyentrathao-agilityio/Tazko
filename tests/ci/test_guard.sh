#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
GUARD="$ROOT/scripts/hooks/guard-write.py"
PY=${PYTHON:-python3}
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
pass=0; fail=0

call() {
  "$PY" -c 'import json,sys; print(json.dumps({"tool_name": sys.argv[1], "tool_input": {sys.argv[2]: sys.argv[3]}}))' "$@" \
    | CLAUDE_PROJECT_DIR="$ROOT" "$PY" "$GUARD" >/dev/null 2>&1
}
expect() {
  local want=$1 name=$2; shift 2
  if call "$@"; then got=allow; else got=block; fi
  if [ "$got" = "$want" ]; then pass=$((pass+1)); echo "  ok   $name"
  else fail=$((fail+1)); echo "  BAD  $name (wanted $want, got $got)"; fi
}

migration=$(cd "$ROOT" && git ls-files 'migrations/tenant/*.sql' | head -1)

expect block "edit a design file" Edit file_path "$ROOT/design/tokens.css"
expect block "write a design file by relative path" Write file_path design/README.md
expect block "edit a committed migration" Edit file_path "$ROOT/$migration"
expect allow "write a new migration" Write file_path "$ROOT/migrations/tenant/9999_new.sql"
expect allow "edit source" Edit file_path "$ROOT/src/main.rs"
expect block "shell redirect into design" Bash command "echo x > design/tokens.css"
expect block "sed in place on design" Bash command "sed -i s/a/b/ design/README.md"
expect block "move a committed migration" Bash command "mv $migration /tmp/x.sql"
expect allow "read design" Bash command "grep -n accent design/tokens.css"
expect allow "read a migration" Bash command "cat $migration"
expect allow "ordinary command" Bash command "cargo test 2>&1 | tail -5"
expect block "remove the migrations folder" Bash command "rm -r migrations/tenant"
expect block "sed in place on migrations by glob" Bash command "sed -i s/a/b/ migrations/tenant/*.sql"
expect block "check out the migrations folder" Bash command "git checkout -- migrations/"
expect block "edit design after cd" Bash command "cd design && sed -i s/a/b/ tokens.css"
expect block "python writes into design" Bash command "python3 -c \"import pathlib; pathlib.Path('design/x').write_text('')\""
expect block "owner-only design update" Bash command "python3 scripts/checks.py design --update"
expect allow "migrations update" Bash command "python3 scripts/checks.py migrations --update"
expect allow "search output for design" Bash command "cargo test 2>&1 | grep -n design/"
expect allow "save a migration elsewhere" Bash command "git show HEAD:$migration > /tmp/x.sql"
expect allow "new migration by shell" Bash command "printf x > migrations/tenant/9999_new.sql"
expect block "edit the guard" Edit file_path "$ROOT/scripts/hooks/guard-write.py"
expect block "edit a hash list" Write file_path "$ROOT/scripts/migrations.sha256"
ln -s "$ROOT/design" "$TMP/linked"
expect block "edit design through a symlink" Edit file_path "$TMP/linked/tokens.css"

echo "  $pass passed, $fail failed"
[ "$fail" -eq 0 ]

#!/usr/bin/env bash
# Tazko checks. Run before every commit: scripts/ci.sh
# Stages that need code that does not exist yet (Rust, web app) skip themselves.
set -euo pipefail
cd "$(dirname "$0")/.."

PY=${PYTHON:-python3}
failed=0
step() { printf '\n== %s\n' "$1"; }
run() { if ! "$@"; then failed=1; fi; }

step "repository files";     run "$PY" scripts/checks.py required
step "clean-room names";     run "$PY" scripts/checks.py names
step "secrets";              run "$PY" scripts/checks.py secrets
step "commits";              run "$PY" scripts/checks.py commits
step "no comments in source"; run "$PY" scripts/checks.py comments
step "one company per file";  run "$PY" scripts/checks.py tenant-schema
step "prototype-only controls"; run "$PY" scripts/checks.py prototype-only
step "design is read-only";   run "$PY" scripts/checks.py design
step "agent settings";        run "$PY" scripts/checks.py settings
step "javascript budget";    run "$PY" scripts/checks.py js-budget web/dist
step "tests for the checks"; run bash tests/ci/test_checks.sh
step "local dev target";     run bash scripts/dev-env.sh check
step "tests for dev-env";    run bash tests/ci/test_dev_env.sh
step "tests for deploy";     run bash tests/ci/test_deploy.sh

if [ -f web/package.json ]; then
  step "web"
  (cd web && pnpm install --frozen-lockfile && pnpm run lint && pnpm run typecheck && pnpm test && pnpm run build) || failed=1
  run "$PY" scripts/checks.py js-budget web/dist
  lic=$(mktemp -d)
  if (cd web && pnpm licenses list --prod --json >"$lic/prod.json" && pnpm licenses list --json >"$lic/all.json"); then
    run "$PY" scripts/checks.py js-licenses "$lic/prod.json" "$lic/all.json"
  else
    failed=1
  fi
  rm -rf "$lic"
else
  printf '== web: skip, no web/package.json yet\n'
fi

if [ -f Cargo.toml ]; then
  step "rust"
  export TAZKO_SKIP_WEB_BUILD=1
  run cargo fmt --check
  run cargo clippy --all-targets -- -D warnings
  run cargo test
  run cargo deny --log-level error check licenses bans sources
else
  printf '\n== rust: skip, no Cargo.toml yet\n'
fi

if [ "$failed" -ne 0 ]; then
  printf '\nci: FAILED\n'
  exit 1
fi
printf '\nci: ok\n'

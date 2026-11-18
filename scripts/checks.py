#!/usr/bin/env python3
"""Repository rule checks for Tazko, run by scripts/ci.sh.

Usage: checks.py <check> [args]
  required            the handoff files exist
  names               no other product is named in a tracked file
  secrets             no secret or settings file is tracked
  js-budget [dir]     shipped JavaScript is under the budget (default web/dist)
  js-licenses <prod.json> <all.json>
                      web dependencies use allowed licences (pnpm licenses list --json)
  commits             every commit is by the owner and carries no attribution lines
  commit-msg <file>   one commit message carries no attribution lines

The word lists are stored as short SHA-256 prefixes so that this file does not
itself name the products it keeps out of the repository.
"""
import gzip
import hashlib
import os
import re
import subprocess
import sys

REQUIRED = [
    "CLAUDE.md",
    "deny.toml",
    "plan.md",
    ".gitignore",
    ".claude/settings.json",
    "design/tazko-prototype.html",
    "design/tokens.css",
    "design/README.md",
    "design/CHANGELOG.md",
    "scripts/ci.sh",
]

# Other work trackers. Checked in every tracked file and every commit message.
PRODUCT_HASHES = {
    "1768c21fc46884df", "1e32847769dc4a15", "4de4a07ad031569c", "c0d67255424148d8",
    "0549e836108aab45", "5f3e3024675ba4b8", "651b2c67d5361363", "6cf5e6317a236541",
    "40c57bb53c07338b", "c106caacaa6923b0", "8bb78a7b3a835919", "2341295f26db521c",
    "7267391c427b5cef", "615dcf411e567417", "f00fb5a3f76c63ec", "282a6dc5066d4725",
}
# Code-generation tools and their vendors. Checked in commit messages.
TOOL_HASHES = {
    "c857d09db23e6822", "c70eca6b0f88f44d", "7d3194f79e645c42",
    "60965168ce762e94", "3ea125d0bff386e6", "053ea4804ef1bb33",
}

ATTRIBUTION = [
    re.compile(r"^\s*co-authored-by\s*:", re.I | re.M),
    re.compile(r"generated\s+(with|by|using)", re.I),
    re.compile(r"\U0001F916"),  # robot face
    re.compile(r"^\s*[A-Za-z-]+-session\s*:", re.I | re.M),
]

SECRET_PATTERNS = [
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    re.compile(r"GOCSPX-[A-Za-z0-9_-]{20,}"),
    re.compile(r"AIza[0-9A-Za-z_-]{35}"),
    re.compile(r"(?:xox[abpr]|ghp|glpat)-?[A-Za-z0-9_-]{20,}"),
    re.compile(r"^[ \t]*TAZKO_(?:GOOGLE_CLIENT_SECRET|SMTP_PASSWORD|SECRET)[ \t]*=[ \t]*\S{8,}", re.M),
]
SECRET_FILES = re.compile(r"(^|/)(\.env(\..*)?|.*\.env|secret|.*\.pem|.*\.key|.*\.db(-wal|-shm)?)$")

JS_BUDGET_BYTES = 200 * 1024  # gzip-compressed size of all shipped .js


def h(word):
    return hashlib.sha256(word.encode()).hexdigest()[:16]


def words(text):
    return {w for w in re.split(r"[^a-z0-9]+", text.lower()) if len(w) >= 3}


def hits(text, hashes):
    return sorted(w for w in words(text) if h(w) in hashes)


def git(*args, cwd="."):
    return subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True, text=True).stdout


def tracked_files():
    # Tracked plus staged-but-new files, so the check runs before the first commit.
    out = git("ls-files", "-z", "--cached", "--others", "--exclude-standard")
    return [p for p in out.split("\0") if p and os.path.isfile(p)]


def fail(msgs):
    for m in msgs:
        print("  FAIL " + m)
    return 1 if msgs else 0


def check_required():
    return fail([f"missing {p}" for p in REQUIRED if not os.path.isfile(p)])


def check_names():
    bad = []
    for p in tracked_files():
        found = hits(p, PRODUCT_HASHES)
        try:
            with open(p, encoding="utf-8", errors="ignore") as f:
                found += hits(f.read(), PRODUCT_HASHES)
        except OSError:
            continue
        if found:
            bad.append(f"{p}: names another product ({len(set(found))} word(s); see the clean-room rules)")
    return fail(bad)


def check_secrets():
    bad = []
    for p in tracked_files():
        if SECRET_FILES.search(p) and not p.endswith(".example"):
            bad.append(f"{p}: settings, key or database files must not be committed")
            continue
        try:
            with open(p, encoding="utf-8", errors="ignore") as f:
                text = f.read()
        except OSError:
            continue
        for pat in SECRET_PATTERNS:
            if pat.search(text):
                bad.append(f"{p}: looks like it contains a secret")
                break
    return fail(bad)


def check_js_budget(root="web/dist"):
    if not os.path.isdir(root):
        print(f"  skip js-budget: {root} not built yet")
        return 0
    total = 0
    for d, _, files in os.walk(root):
        for n in files:
            if n.endswith((".js", ".mjs")):
                with open(os.path.join(d, n), "rb") as f:
                    total += len(gzip.compress(f.read(), 9))
    print(f"  js-budget: {total / 1024:.1f} KB gzip of {JS_BUDGET_BYTES // 1024} KB")
    return fail([f"shipped JavaScript is {total} bytes gzip, over {JS_BUDGET_BYTES}"] if total > JS_BUDGET_BYTES else [])


ALLOWED_LICENSES = {"MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC"}
BUILD_TOOL_EXCEPTIONS = {
    "lightningcss": "MPL-2.0",
    "lru-cache": "BlueOak-1.0.0",
    "graceful-fs": "BlueOak-1.0.0",
}
UNBUNDLED_EXCEPTIONS = {
    "tslib": "0BSD",
}


def licence_ok(expr, allowed):
    expr = expr.strip()
    while expr.startswith("(") and expr.endswith(")"):
        expr = expr[1:-1].strip()
    if " OR " in expr:
        return any(licence_ok(part, allowed) for part in expr.split(" OR "))
    if " AND " in expr:
        return all(licence_ok(part, allowed) for part in expr.split(" AND "))
    return expr in allowed


def build_tool_exception(name):
    for base, licence in BUILD_TOOL_EXCEPTIONS.items():
        if name == base or name.startswith(base + "-"):
            return licence
    return None


def load_licences(path):
    import json

    with open(path, encoding="utf-8") as f:
        data = json.load(f)
    return [(pkg["name"], licence) for licence, pkgs in data.items() for pkg in pkgs]


def check_js_licenses(prod_path, all_path):
    bad = []
    prod = load_licences(prod_path)
    for name, licence in prod:
        if UNBUNDLED_EXCEPTIONS.get(name) == licence:
            continue
        if not licence_ok(licence, ALLOWED_LICENSES):
            bad.append(f"{name} ships in the web app under {licence}")
    shipped = {n for n, _ in prod}
    for name, licence in load_licences(all_path):
        if name in shipped or licence_ok(licence, ALLOWED_LICENSES):
            continue
        if build_tool_exception(name) == licence:
            continue
        bad.append(f"{name} ({licence}) is not an allowed licence or a listed build-tool exception")
    print(f"  js-licenses: {len(prod)} shipped packages checked")
    return fail(bad)


GITHUB_MERGE_COMMITTER = ("GitHub", "noreply@github.com")


def message_problems(msg):
    bad = [f"attribution line matching /{p.pattern}/" for p in ATTRIBUTION if p.search(msg)]
    if hits(msg, TOOL_HASHES | PRODUCT_HASHES):
        bad.append("names a tool or another product")
    return bad


def check_commit_msg(path):
    with open(path, encoding="utf-8") as f:
        msg = "\n".join(l for l in f.read().splitlines() if not l.startswith("#"))
    return fail(message_problems(msg))


def check_commits():
    try:
        git("rev-parse", "--verify", "-q", "HEAD")
    except subprocess.CalledProcessError:
        print("  skip commits: no commits yet")
        return 0
    owner_name = os.environ.get("TAZKO_OWNER_NAME") or git("config", "user.name").strip()
    owner_email = os.environ.get("TAZKO_OWNER_EMAIL") or git("config", "user.email").strip()
    log = git("log", "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%cn%x1f%ce%x1f%B%x1e")
    bad = []
    for rec in filter(str.strip, log.split("\x1e")):
        sha, parents, an, ae, cn, ce, body = rec.strip("\n").split("\x1f", 6)
        short = sha[:8]
        if len(parents.split()) > 1 and (cn, ce.lower()) == GITHUB_MERGE_COMMITTER:
            bad += [f"{short}: {p}" for p in message_problems(body)]
            continue
        if (an, ae.lower()) != (owner_name, owner_email.lower()):
            bad.append(f"{short}: author {an} <{ae}> is not the owner {owner_name} <{owner_email}>")
        if (cn, ce.lower()) != (owner_name, owner_email.lower()):
            bad.append(f"{short}: committer {cn} <{ce}> is not the owner")
        bad += [f"{short}: {p}" for p in message_problems(body)]
    return fail(bad)


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    cmd, rest = argv[1], argv[2:]
    table = {
        "required": check_required,
        "names": check_names,
        "secrets": check_secrets,
        "js-budget": check_js_budget,
        "js-licenses": check_js_licenses,
        "commits": check_commits,
        "commit-msg": check_commit_msg,
    }
    if cmd not in table:
        print(__doc__)
        return 2
    return table[cmd](*rest)


if __name__ == "__main__":
    sys.exit(main(sys.argv))

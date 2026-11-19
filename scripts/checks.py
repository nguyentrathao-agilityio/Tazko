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
  comments            no comments in src/, web/src/ or migrations/
  tenant-schema       no company column in the company file migrations
  prototype-only      the prototype-only controls are not built
  design [--update]   design/ matches scripts/design.sha256 (--update: owner only)
  settings            .claude/settings.json keeps attribution off and machine commands on ask

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

OWNER_EMAIL_HASHES = {"1caab6a871c4cf3e"}

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


def email_hash(email):
    return hashlib.sha256(email.strip().lower().encode()).hexdigest()[:16]


def owner_matcher():
    name = os.environ.get("TAZKO_OWNER_NAME")
    email = os.environ.get("TAZKO_OWNER_EMAIL")
    if email:
        return (lambda n, e: (not name or n == name) and e.lower() == email.lower()), f"{name or '*'} <{email}>"
    return (lambda n, e: email_hash(e) in OWNER_EMAIL_HASHES), "the owner's email in OWNER_EMAIL_HASHES"


def check_commits():
    try:
        git("rev-parse", "--verify", "-q", "HEAD")
    except subprocess.CalledProcessError:
        print("  skip commits: no commits yet")
        return 0
    is_owner, owner = owner_matcher()
    log = git("log", "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%cn%x1f%ce%x1f%B%x1e")
    bad = []
    for rec in filter(str.strip, log.split("\x1e")):
        sha, parents, an, ae, cn, ce, body = rec.strip("\n").split("\x1f", 6)
        short = sha[:8]
        if len(parents.split()) > 1 and (cn, ce.lower()) == GITHUB_MERGE_COMMITTER:
            bad += [f"{short}: {p}" for p in message_problems(body)]
            continue
        if not is_owner(an, ae):
            bad.append(f"{short}: author {an} <{ae}> is not {owner}; commit with the owner's git identity")
        if not is_owner(cn, ce):
            bad.append(f"{short}: committer {cn} <{ce}> is not {owner}; commit with the owner's git identity")
        bad += [f"{short}: {p}" for p in message_problems(body)]
    return fail(bad)


COMMENT_ROOTS = ("src/", "web/src/", "migrations/")
LINE_MARKERS = {".rs": "//", ".ts": "//", ".tsx": "//", ".js": "//", ".sql": "--"}
BLOCK_COMMENT_EXTS = {".rs", ".ts", ".tsx", ".js", ".css", ".sql"}
COMMENT_ALLOWED = re.compile(r"^\s*///\s*<reference\s")
STRING_LITERAL = re.compile(r'"(?:\\.|[^"\\])*"|`(?:\\.|[^`\\])*`|\'(?:\\.|[^\'\\\n])*\'')


def comment_on_line(line, ext):
    if COMMENT_ALLOWED.match(line):
        return False
    code = STRING_LITERAL.sub('""', line.replace("\\/", ""))
    marker = LINE_MARKERS.get(ext)
    if marker and marker in code:
        return True
    return ext in BLOCK_COMMENT_EXTS and "/*" in code


def check_comments():
    bad = []
    for p in tracked_files():
        ext = os.path.splitext(p)[1]
        if not p.startswith(COMMENT_ROOTS) or (ext not in LINE_MARKERS and ext not in BLOCK_COMMENT_EXTS):
            continue
        with open(p, encoding="utf-8", errors="ignore") as f:
            for n, line in enumerate(f, 1):
                if comment_on_line(line, ext):
                    bad.append(f"{p}:{n}: comment in source; name things so the code reads without it, "
                               "and put the reason in the commit message or docs/")
    return fail(bad)


TENANT_COLUMN = re.compile(r"\b(?:org|orgs|company|companies|tenant|tenants)_(?:id|slug|key|uuid)\b", re.I)


def check_tenant_schema(root="migrations/tenant"):
    bad = []
    if not os.path.isdir(root):
        print(f"  skip tenant-schema: no {root}")
        return 0
    for n in sorted(os.listdir(root)):
        p = os.path.join(root, n)
        if not n.endswith(".sql"):
            continue
        with open(p, encoding="utf-8") as f:
            for i, line in enumerate(f, 1):
                m = TENANT_COLUMN.search(line)
                if m:
                    bad.append(f"{p}:{i}: '{m.group(0)}' in a company file; each company has its own "
                               "tazko.db, so open that company's file instead of adding a company column")
    return fail(bad)


PROTOTYPE_ONLY = ["View as", "Live sim"]


def check_prototype_only(root="web/src/"):
    bad = []
    for p in tracked_files():
        if not p.startswith(root):
            continue
        with open(p, encoding="utf-8", errors="ignore") as f:
            text = f.read()
        for label in PROTOTYPE_ONLY:
            if label in text:
                bad.append(f"{p}: '{label}' is a prototype-only control; build the role differences "
                           "on the server and leave the prototype bar out")
    return fail(bad)


DESIGN_DIR = "design/"
DESIGN_LIST = "scripts/design.sha256"


def design_hashes():
    out = {}
    for p in sorted(tracked_files()):
        if p.startswith(DESIGN_DIR):
            with open(p, "rb") as f:
                out[p] = hashlib.sha256(f.read()).hexdigest()
    return out


def check_design(*args):
    current = design_hashes()
    if args == ("--update",):
        with open(DESIGN_LIST, "w", encoding="utf-8") as f:
            f.writelines(f"{d}  {p}\n" for p, d in current.items())
        print(f"  design: {len(current)} file(s) written to {DESIGN_LIST}")
        return 0
    if args:
        print(__doc__)
        return 2
    if not os.path.isfile(DESIGN_LIST):
        return fail([f"missing {DESIGN_LIST}; the owner creates it with checks.py design --update"])
    saved = {}
    with open(DESIGN_LIST, encoding="utf-8") as f:
        for line in f:
            if line.strip():
                d, p = line.split(None, 1)
                saved[p.strip()] = d
    hint = "design/ is read-only for the build; ask the owner, who runs checks.py design --update after the change"
    bad = [f"{p}: changed; {hint}" for p in current if p in saved and saved[p] != current[p]]
    bad += [f"{p}: added; {hint}" for p in current if p not in saved]
    bad += [f"{p}: removed; {hint}" for p in saved if p not in current]
    return fail(bad)


SETTINGS_FILE = ".claude/settings.json"
REQUIRED_ASK = [
    "Bash(sudo:*)",
    "Bash(systemctl:*)",
    "Bash(cloudflared:*)",
    "Bash(apt:*)",
    "Bash(apt-get:*)",
    "Bash(brew install:*)",
    "Bash(useradd:*)",
    "Bash(usermod:*)",
    "Bash(ssh:*)",
    "Bash(scp:*)",
]


def check_settings(path=SETTINGS_FILE):
    import json

    try:
        with open(path, encoding="utf-8") as f:
            data = json.load(f)
    except (OSError, ValueError) as e:
        return fail([f"{path}: cannot read ({e})"])
    bad = []
    attribution = data.get("attribution", {})
    if attribution.get("commit") != "" or attribution.get("pr") != "":
        bad.append(f"{path}: attribution.commit and attribution.pr must stay empty strings")
    if data.get("includeCoAuthoredBy") is not False:
        bad.append(f"{path}: includeCoAuthoredBy must stay false")
    perms = data.get("permissions", {})
    ask = perms.get("ask", [])
    allow = perms.get("allow", [])
    for rule in REQUIRED_ASK:
        if rule not in ask:
            bad.append(f"{path}: permissions.ask must keep {rule}; machine changes wait for the owner")
        if rule in allow:
            bad.append(f"{path}: {rule} must not be in permissions.allow")
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
        "comments": check_comments,
        "tenant-schema": check_tenant_schema,
        "prototype-only": check_prototype_only,
        "design": check_design,
        "settings": check_settings,
    }
    if cmd not in table:
        print(__doc__)
        return 2
    return table[cmd](*rest)


if __name__ == "__main__":
    sys.exit(main(sys.argv))

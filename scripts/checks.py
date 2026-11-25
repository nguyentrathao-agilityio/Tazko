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
  prototype-only      the prototype-only controls are not built
  web-paths           web/src builds API and route paths only through web/src/paths.ts
  design [--update]   design/ matches scripts/design.sha256 (--update: owner only)
  migrations [--update]
                      committed migrations match scripts/migrations.sha256
                      (--update adds new migrations, never changes listed ones)
  settings            .claude/settings.json keeps attribution off, machine commands on ask
                      and the write guard hook on

The word lists are stored as short SHA-256 prefixes so that this file does not
itself name the products it keeps out of the repository.
"""
from __future__ import annotations

import gzip
import hashlib
import importlib.util
import inspect
import json
import os
import re
import subprocess
import sys
from collections.abc import Callable, Iterable

EXIT_OK = 0
EXIT_FAILED = 1
EXIT_USAGE = 2

ENCODING = "utf-8"
HASH_PREFIX_LENGTH = 16
SHORT_SHA_LENGTH = 8
KB = 1024

SRC_DIR = "src/"
WEB_SRC_DIR = "web/src/"
WEB_DIST_DIR = "web/dist"
MIGRATIONS_DIR = "migrations/"
DESIGN_DIR = "design/"
DESIGN_LIST = "scripts/design.sha256"
MIGRATIONS_LIST = "scripts/migrations.sha256"
SETTINGS_FILE = ".claude/settings.json"

REQUIRED = (
    "CLAUDE.md",
    "deny.toml",
    "plan.md",
    ".gitignore",
    SETTINGS_FILE,
    DESIGN_DIR + "tazko-prototype.html",
    DESIGN_DIR + "tokens.css",
    DESIGN_DIR + "README.md",
    DESIGN_DIR + "CHANGELOG.md",
    "scripts/ci.sh",
)

OTHER_WORK_TRACKER_HASHES = frozenset({
    "1768c21fc46884df", "1e32847769dc4a15", "4de4a07ad031569c", "c0d67255424148d8",
    "0549e836108aab45", "5f3e3024675ba4b8", "651b2c67d5361363", "6cf5e6317a236541",
    "40c57bb53c07338b", "c106caacaa6923b0", "8bb78a7b3a835919", "2341295f26db521c",
    "7267391c427b5cef", "615dcf411e567417", "f00fb5a3f76c63ec", "282a6dc5066d4725",
})
CODE_GENERATION_TOOL_HASHES = frozenset({
    "c857d09db23e6822", "c70eca6b0f88f44d", "7d3194f79e645c42",
    "60965168ce762e94", "3ea125d0bff386e6", "053ea4804ef1bb33",
})
OWNER_EMAIL_HASHES = frozenset({"1caab6a871c4cf3e"})
MIN_WORD_LENGTH = 3

ROBOT_FACE = "\U0001F916"
ATTRIBUTION = (
    re.compile(r"^\s*co-authored-by\s*:", re.I | re.M),
    re.compile(r"generated\s+(with|by|using)", re.I),
    re.compile(ROBOT_FACE),
    re.compile(r"^\s*[A-Za-z-]+-session\s*:", re.I | re.M),
)

SECRET_PATTERNS = (
    re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----"),
    re.compile(r"GOCSPX-[A-Za-z0-9_-]{20,}"),
    re.compile(r"AIza[0-9A-Za-z_-]{35}"),
    re.compile(r"(?:xox[abpr]|ghp|glpat)-?[A-Za-z0-9_-]{20,}"),
    re.compile(r"^[ \t]*TAZKO_(?:GOOGLE_CLIENT_SECRET|SMTP_PASSWORD|SECRET)[ \t]*=[ \t]*\S{8,}", re.M),
    re.compile(r"^[ \t]*(?:export[ \t]+)?DUCKDNS_TOKEN[ \t]*[:=][ \t]*[\"']?[^\s\"']{8,}", re.M),
    re.compile(r"duckdns\.org/update\S*token=[0-9a-f]{8}-[0-9a-f]{4}"),
)
SECRET_FILES = re.compile(r"(^|/)(\.env(\..*)?|.*\.env|secret|.*\.pem|.*\.key|.*\.db(-wal|-shm)?)$")
EXAMPLE_SUFFIX = ".example"

JS_EXTENSIONS = (".js", ".mjs")
JS_BUDGET_GZIP_BYTES = 200 * KB
GZIP_LEVEL = 9

ALLOWED_LICENCES = frozenset({"MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC"})
BUILD_TOOL_EXCEPTIONS = {
    "lightningcss": "MPL-2.0",
    "lru-cache": "BlueOak-1.0.0",
    "graceful-fs": "BlueOak-1.0.0",
}
UNBUNDLED_EXCEPTIONS = {
    "tslib": "0BSD",
}
LICENCE_TOKEN = re.compile(r"\(|\)|[^\s()]+")

GITHUB_MERGE_COMMITTER = ("GitHub", "noreply@github.com")
OWNER_IDENTITY_HINT = "commit with the owner's git identity"

COMMENT_ROOTS = (SRC_DIR, WEB_SRC_DIR, MIGRATIONS_DIR)
LINE_COMMENT_MARKERS = {".rs": "//", ".ts": "//", ".tsx": "//", ".js": "//", ".sql": "--"}
BLOCK_COMMENT_EXTS = frozenset({".rs", ".ts", ".tsx", ".js", ".css", ".sql"})
BLOCK_COMMENT_MARKER = "/*"
ESCAPED_SLASH = "\\/"
COMMENT_ALLOWED = re.compile(r"^\s*///\s*<reference\s")
STRING_LITERAL = re.compile(r""""(?:\\.|[^"\\])*"|`(?:\\.|[^`\\])*`|'(?:\\.|[^'\\\n])*'""")
COMMENT_HINT = "comment in source; name things so the code reads without it, and put the reason in the commit message or docs/"

PROTOTYPE_ONLY_LABELS = ("View as", "Live sim")
WEB_SOURCE_EXTS = (".ts", ".tsx", ".js", ".json", ".css", ".html")
JS_SOURCE_EXTS = (".ts", ".tsx", ".js", ".jsx", ".mjs")
WEB_PATHS_FILE = WEB_SRC_DIR + "paths.ts"
RAW_WEB_PATH = re.compile(r"['\"`]/api/v1\b|['\"`]/c/\$\{|\}/c/\$\{")
WEB_PATHS_HINT = f"builds a path by hand; add or use a helper in {WEB_PATHS_FILE}"
TEST_FILE_MARK = ".test."
PROTOTYPE_HINT = "is a prototype-only control; build the role differences on the server and leave the prototype bar out"

DESIGN_HINT = "design/ is read-only for the build; ask the owner, who runs checks.py design --update after the change"
MIGRATIONS_HINT = "committed migrations never change; add a new numbered migration instead"
MIGRATIONS_LIST_HINT = "run checks.py migrations --update to list it"
UPDATE_FLAG = "--update"

MACHINE_COMMANDS = (
    "sudo",
    "systemctl",
    "cloudflared",
    "apt",
    "apt-get",
    "brew install",
    "useradd",
    "usermod",
    "ssh",
    "scp",
)
REQUIRED_ASK = tuple(f"Bash({command}:*)" for command in MACHINE_COMMANDS)
GUARD_HOOK_SCRIPT = "scripts/hooks/guard-write.py"
MATCH_EVERY_TOOL = ("", "*")


def short_hash(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()[:HASH_PREFIX_LENGTH]


def words(text: str) -> set[str]:
    return {w for w in re.split(r"[^a-z0-9]+", text.lower()) if len(w) >= MIN_WORD_LENGTH}


def hits(text: str, hashes: frozenset[str]) -> list[str]:
    return sorted(w for w in words(text) if short_hash(w) in hashes)


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True).stdout


def tracked_files() -> list[str]:
    out = git("ls-files", "-z", "--cached", "--others", "--exclude-standard")
    return [p for p in out.split("\0") if p and os.path.isfile(p)]


def files_under(*prefixes: str) -> list[str]:
    return [p for p in tracked_files() if p.startswith(prefixes)]


def read_text(path: str) -> str:
    with open(path, encoding=ENCODING, errors="ignore") as f:
        return f.read()


def read_bytes(path: str) -> bytes:
    with open(path, "rb") as f:
        return f.read()


def read_json(path: str):
    with open(path, encoding=ENCODING) as f:
        return json.load(f)


def numbered_lines(path: str) -> Iterable[tuple[int, str]]:
    return enumerate(read_text(path).splitlines(), 1)


def fail(messages: Iterable[str]) -> int:
    messages = list(messages)
    for message in messages:
        print("  FAIL " + message)
    return EXIT_FAILED if messages else EXIT_OK


def skip(check: str, reason: str) -> int:
    print(f"  skip {check}: {reason}")
    return EXIT_OK


def check_required() -> int:
    return fail(f"missing {p}" for p in REQUIRED if not os.path.isfile(p))


def product_words(path: str) -> set[str]:
    try:
        text = path + "\n" + read_text(path)
    except OSError:
        return set()
    return set(hits(text, OTHER_WORK_TRACKER_HASHES))


def check_names() -> int:
    return fail(
        f"{p}: names another product ({len(found)} word(s); see the clean-room rules)"
        for p in tracked_files()
        if (found := product_words(p))
    )


def secret_problem(path: str) -> str | None:
    if SECRET_FILES.search(path) and not path.endswith(EXAMPLE_SUFFIX):
        return "settings, key or database files must not be committed"
    try:
        text = read_text(path)
    except OSError:
        return None
    if any(pattern.search(text) for pattern in SECRET_PATTERNS):
        return "looks like it contains a secret"
    return None


def check_secrets() -> int:
    return fail(f"{p}: {problem}" for p in tracked_files() if (problem := secret_problem(p)))


def gzip_size(path: str) -> int:
    return len(gzip.compress(read_bytes(path), GZIP_LEVEL))


def check_js_budget(root: str = WEB_DIST_DIR) -> int:
    if not os.path.isdir(root):
        return skip("js-budget", f"{root} not built yet")
    total = sum(
        gzip_size(os.path.join(folder, name))
        for folder, _, names in os.walk(root)
        for name in names
        if name.endswith(JS_EXTENSIONS)
    )
    print(f"  js-budget: {total / KB:.1f} KB gzip of {JS_BUDGET_GZIP_BYTES // KB} KB")
    over_budget = total > JS_BUDGET_GZIP_BYTES
    return fail([f"shipped JavaScript is {total} bytes gzip, over {JS_BUDGET_GZIP_BYTES}"] if over_budget else [])


def licence_ok(expression: str, allowed: frozenset[str]) -> bool:
    tokens = LICENCE_TOKEN.findall(expression)
    position = 0

    def peek() -> str | None:
        return tokens[position] if position < len(tokens) else None

    def take() -> str:
        nonlocal position
        position += 1
        return tokens[position - 1]

    def any_of() -> bool:
        result = all_of()
        while peek() == "OR":
            take()
            result = all_of() or result
        return result

    def all_of() -> bool:
        result = single()
        while peek() == "AND":
            take()
            result = single() and result
        return result

    def single() -> bool:
        token = take() if peek() is not None else ""
        if token == "(":
            result = any_of()
            if peek() == ")":
                take()
            return result
        return token in allowed

    result = any_of()
    return result and position == len(tokens)


def build_tool_exception(name: str) -> str | None:
    return next(
        (licence for base, licence in BUILD_TOOL_EXCEPTIONS.items() if name == base or name.startswith(base + "-")),
        None,
    )


def load_licences(path: str) -> list[tuple[str, str]]:
    return [(pkg["name"], licence) for licence, pkgs in read_json(path).items() for pkg in pkgs]


def check_js_licenses(prod_path: str, all_path: str) -> int:
    prod = load_licences(prod_path)
    shipped = {name for name, _ in prod}
    bad = [
        f"{name} ships in the web app under {licence}"
        for name, licence in prod
        if UNBUNDLED_EXCEPTIONS.get(name) != licence and not licence_ok(licence, ALLOWED_LICENCES)
    ]
    bad += [
        f"{name} ({licence}) is not an allowed licence or a listed build-tool exception"
        for name, licence in load_licences(all_path)
        if name not in shipped
        and not licence_ok(licence, ALLOWED_LICENCES)
        and build_tool_exception(name) != licence
    ]
    print(f"  js-licenses: {len(prod)} shipped packages checked")
    return fail(bad)


def message_problems(message: str) -> list[str]:
    bad = [f"attribution line matching /{p.pattern}/" for p in ATTRIBUTION if p.search(message)]
    if hits(message, CODE_GENERATION_TOOL_HASHES | OTHER_WORK_TRACKER_HASHES):
        bad.append("names a tool or another product")
    return bad


def check_commit_msg(path: str) -> int:
    message = "\n".join(line for line in read_text(path).splitlines() if not line.startswith("#"))
    return fail(message_problems(message))


def owner_matcher() -> tuple[Callable[[str, str], bool], str]:
    name = os.environ.get("TAZKO_OWNER_NAME")
    email = os.environ.get("TAZKO_OWNER_EMAIL")
    if email:
        return (lambda n, e: (not name or n == name) and e.lower() == email.lower()), f"{name or '*'} <{email}>"
    return (lambda n, e: short_hash(e.strip().lower()) in OWNER_EMAIL_HASHES), "the owner's email in OWNER_EMAIL_HASHES"


def has_commits() -> bool:
    try:
        git("rev-parse", "--verify", "-q", "HEAD")
    except subprocess.CalledProcessError:
        return False
    return True


def commit_records() -> Iterable[list[str]]:
    log = git("log", "--format=%H%x1f%P%x1f%an%x1f%ae%x1f%cn%x1f%ce%x1f%B%x1e")
    for record in filter(str.strip, log.split("\x1e")):
        yield record.strip("\n").split("\x1f", 6)


def is_github_merge(parents: str, committer: str, committer_email: str) -> bool:
    return len(parents.split()) > 1 and (committer, committer_email.lower()) == GITHUB_MERGE_COMMITTER


def commit_problems(record: list[str], is_owner: Callable[[str, str], bool], owner: str) -> list[str]:
    sha, parents, author, author_email, committer, committer_email, body = record
    bad = []
    if not is_github_merge(parents, committer, committer_email):
        if not is_owner(author, author_email):
            bad.append(f"author {author} <{author_email}> is not {owner}; {OWNER_IDENTITY_HINT}")
        if not is_owner(committer, committer_email):
            bad.append(f"committer {committer} <{committer_email}> is not {owner}; {OWNER_IDENTITY_HINT}")
    bad += message_problems(body)
    return [f"{sha[:SHORT_SHA_LENGTH]}: {problem}" for problem in bad]


def check_commits() -> int:
    if not has_commits():
        return skip("commits", "no commits yet")
    is_owner, owner = owner_matcher()
    return fail(problem for record in commit_records() for problem in commit_problems(record, is_owner, owner))


def has_comment(line: str, ext: str) -> bool:
    if COMMENT_ALLOWED.match(line):
        return False
    code = STRING_LITERAL.sub('""', line.replace(ESCAPED_SLASH, ""))
    marker = LINE_COMMENT_MARKERS.get(ext)
    return (marker is not None and marker in code) or (ext in BLOCK_COMMENT_EXTS and BLOCK_COMMENT_MARKER in code)


def check_comments() -> int:
    return fail(
        f"{p}:{n}: {COMMENT_HINT}"
        for p in files_under(*COMMENT_ROOTS)
        if (ext := os.path.splitext(p)[1]) in BLOCK_COMMENT_EXTS
        for n, line in numbered_lines(p)
        if has_comment(line, ext)
    )


def check_prototype_only() -> int:
    return fail(
        f"{p}: '{label}' {PROTOTYPE_HINT}"
        for p in files_under(WEB_SRC_DIR)
        if p.endswith(WEB_SOURCE_EXTS)
        for text in (read_text(p),)
        for label in PROTOTYPE_ONLY_LABELS
        if label in text
    )


def check_web_paths() -> int:
    return fail(
        f"{p}:{n}: {WEB_PATHS_HINT}"
        for p in files_under(WEB_SRC_DIR)
        if p.endswith(JS_SOURCE_EXTS) and p != WEB_PATHS_FILE and TEST_FILE_MARK not in p
        for n, line in numbered_lines(p)
        if RAW_WEB_PATH.search(line)
    )


def file_hashes(prefix: str) -> dict[str, str]:
    return {p: hashlib.sha256(read_bytes(p)).hexdigest() for p in sorted(files_under(prefix))}


def saved_hashes(list_path: str) -> dict[str, str]:
    pairs = (line.split(None, 1) for line in read_text(list_path).splitlines() if line.strip())
    return {path: digest for digest, path in pairs}


def write_hashes(list_path: str, hashes: dict[str, str]) -> None:
    with open(list_path, "w", encoding=ENCODING) as f:
        f.writelines(f"{digest}  {path}\n" for path, digest in sorted(hashes.items()))


def hash_differences(current: dict[str, str], saved: dict[str, str]) -> dict[str, set[str]]:
    return {
        "changed": {p for p in current.keys() & saved.keys() if current[p] != saved[p]},
        "added": current.keys() - saved.keys(),
        "removed": saved.keys() - current.keys(),
    }


def check_design(*args: str) -> int:
    if args and args != (UPDATE_FLAG,):
        return usage()
    current = file_hashes(DESIGN_DIR)
    if args:
        write_hashes(DESIGN_LIST, current)
        print(f"  design: {len(current)} file(s) written to {DESIGN_LIST}")
        return EXIT_OK
    if not os.path.isfile(DESIGN_LIST):
        return fail([f"missing {DESIGN_LIST}; the owner creates it with checks.py design {UPDATE_FLAG}"])
    differences = hash_differences(current, saved_hashes(DESIGN_LIST))
    return fail(f"{p}: {state}; {DESIGN_HINT}" for state, paths in differences.items() for p in sorted(paths))


def check_migrations(*args: str) -> int:
    if args and args != (UPDATE_FLAG,):
        return usage()
    current = file_hashes(MIGRATIONS_DIR)
    saved = saved_hashes(MIGRATIONS_LIST) if os.path.isfile(MIGRATIONS_LIST) else {}
    differences = hash_differences(current, saved)
    broken = [f"{p}: {state}; {MIGRATIONS_HINT}" for state in ("changed", "removed") for p in sorted(differences[state])]
    if args:
        if broken:
            return fail(broken)
        write_hashes(MIGRATIONS_LIST, current)
        print(f"  migrations: {len(differences['added'])} added to {MIGRATIONS_LIST}")
        return EXIT_OK
    return fail(broken + [f"{p}: not listed in {MIGRATIONS_LIST}; {MIGRATIONS_LIST_HINT}" for p in sorted(differences["added"])])


def guarded_tools() -> list[str]:
    path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "hooks", os.path.basename(GUARD_HOOK_SCRIPT))
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("guard_write", path)
    guard = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(guard)
    return sorted(guard.FILE_TOOLS | {guard.SHELL_TOOL})


def has_guard_hook(data: dict, tools: list[str]) -> bool:
    for entry in data.get("hooks", {}).get("PreToolUse", []):
        matcher = entry.get("matcher", "")
        runs_guard = any(GUARD_HOOK_SCRIPT in hook.get("command", "") for hook in entry.get("hooks", []))
        covers_all = matcher in MATCH_EVERY_TOOL or all(re.fullmatch(matcher, tool) for tool in tools)
        if runs_guard and covers_all:
            return True
    return False


def check_settings(path: str = SETTINGS_FILE) -> int:
    try:
        data = read_json(path)
    except (OSError, ValueError) as e:
        return fail([f"{path}: cannot read ({e})"])
    attribution = data.get("attribution", {})
    permissions = data.get("permissions", {})
    ask, allow = permissions.get("ask", []), permissions.get("allow", [])
    bad = []
    if attribution.get("commit") != "" or attribution.get("pr") != "":
        bad.append(f"{path}: attribution.commit and attribution.pr must stay empty strings")
    if data.get("includeCoAuthoredBy") is not False:
        bad.append(f"{path}: includeCoAuthoredBy must stay false")
    bad += [f"{path}: permissions.ask must keep {rule}; machine changes wait for the owner" for rule in REQUIRED_ASK if rule not in ask]
    bad += [f"{path}: {rule} must not be in permissions.allow" for rule in REQUIRED_ASK if rule in allow]
    try:
        tools = guarded_tools()
        guarded = has_guard_hook(data, tools)
    except (OSError, AttributeError, re.error) as e:
        return fail(bad + [f"{path}: cannot check the write guard hook ({e})"])
    if not guarded:
        bad.append(f"{path}: hooks.PreToolUse must run {GUARD_HOOK_SCRIPT} for {', '.join(tools)}")
    return fail(bad)


CHECKS: dict[str, Callable[..., int]] = {
    "required": check_required,
    "names": check_names,
    "secrets": check_secrets,
    "js-budget": check_js_budget,
    "js-licenses": check_js_licenses,
    "commits": check_commits,
    "commit-msg": check_commit_msg,
    "comments": check_comments,
    "prototype-only": check_prototype_only,
    "web-paths": check_web_paths,
    "design": check_design,
    "migrations": check_migrations,
    "settings": check_settings,
}


def usage() -> int:
    print(__doc__)
    return EXIT_USAGE


def main(argv: list[str]) -> int:
    if len(argv) < 2 or argv[1] not in CHECKS:
        return usage()
    check, args = CHECKS[argv[1]], argv[2:]
    try:
        inspect.signature(check).bind(*args)
    except TypeError:
        return usage()
    return check(*args)


if __name__ == "__main__":
    sys.exit(main(sys.argv))

#!/usr/bin/env python3
"""Pre-tool hook for coding agents: refuse writes to design/ and to committed migrations.

Reads the tool call as JSON on stdin. Exit 2 blocks the call and shows the
message to the agent; exit 0 lets it through. The hook catches the common
mistakes early; the real control is CI, where the design and migrations checks
in scripts/checks.py compare every file with its saved hash. scripts/checks.py
reads FILE_TOOLS and SHELL_TOOL from here.
"""
import json
import os
import re
import sys

BLOCK = 2
DESIGN_DIR = "design/"
MIGRATIONS_DIR = "migrations/"
PROTECTED_FILES = frozenset({"scripts/hooks/guard-write.py", "scripts/design.sha256", "scripts/migrations.sha256"})
FILE_TOOLS = frozenset({"Edit", "MultiEdit", "Write", "NotebookEdit"})
SHELL_TOOL = "Bash"
WRITING_PROGRAMS = (
    "tee", "mv", "cp", "rm", "rmdir", "truncate", "touch", "ln", "install", "patch", "dd",
    "python", "python3", "node", "ruby", "perl", "awk",
)
WRITING_COMMAND = re.compile(
    "|".join((
        r">\s*['\"]?[^\s'\"]*(?:design|migrations)/",
        r"(?:^|[\s;&|(])(?:" + "|".join(WRITING_PROGRAMS) + r")\b",
        r"\bsed\s+(?:-[a-zA-Z]*i|--in-place)",
        r"\bgit\s+(?:checkout|restore|apply|rm|mv|reset|stash|clean)\b",
    ))
)
ENTERS_PROTECTED_DIR = re.compile(r"\b(?:cd|pushd)\s+['\"]?(?:\./)?(?:design|migrations)\b")
MIGRATION_PATHSPEC = re.compile(r"(?<![\w:])migrations/[^\s'\";|&<>)]*")
DESIGN_UPDATE = re.compile(r"checks\.py\s+design\s+--update")
DESIGN_HINT = "design/ is read-only for the build; ask the owner to change the prototype"
MIGRATION_HINT = "committed migrations never change; add a new numbered migration instead"
PROTECTED_HINT = "this file keeps the build rules; ask the owner to change it"


def committed(root, pathspecs):
    import subprocess

    result = subprocess.run(["git", "ls-files", "--", *pathspecs], cwd=root, capture_output=True, text=True, check=True)
    return sorted(set(result.stdout.splitlines()))


def repository_of(path):
    folder = os.path.dirname(path)
    while folder != os.path.dirname(folder):
        if os.path.exists(os.path.join(folder, ".git")):
            return folder
        folder = os.path.dirname(folder)
    return None


def file_problem(root, path):
    full = os.path.realpath(os.path.join(root, path))
    repo = repository_of(full) or os.path.realpath(root)
    rel = os.path.relpath(full, repo)
    if rel.startswith(DESIGN_DIR):
        return f"{rel}: {DESIGN_HINT}"
    if rel in PROTECTED_FILES:
        return f"{rel}: {PROTECTED_HINT}"
    if rel.startswith(MIGRATIONS_DIR) and committed(repo, [rel]):
        return f"{rel}: {MIGRATION_HINT}"
    return None


def command_problem(root, command):
    if DESIGN_UPDATE.search(command):
        return f"only the owner updates the design list; {DESIGN_HINT}"
    if DESIGN_DIR not in command and MIGRATIONS_DIR not in command and not ENTERS_PROTECTED_DIR.search(command):
        return None
    if not WRITING_COMMAND.search(command):
        return None
    if DESIGN_DIR in command or re.search(r"\b(?:cd|pushd)\s+['\"]?(?:\./)?design\b", command):
        return f"this command may write to design/; {DESIGN_HINT}"
    pathspecs = MIGRATION_PATHSPEC.findall(command) or [MIGRATIONS_DIR]
    changed = committed(root, pathspecs)
    if changed:
        return f"this command may change {', '.join(changed[:3])}; {MIGRATION_HINT}"
    return None


def problem_for(call):
    tool = call.get("tool_name", "")
    data = call.get("tool_input") or {}
    root = os.environ.get("CLAUDE_PROJECT_DIR") or call.get("cwd") or os.getcwd()
    if tool in FILE_TOOLS:
        return file_problem(root, data.get("file_path") or data.get("notebook_path") or "")
    if tool == SHELL_TOOL:
        return command_problem(root, data.get("command") or "")
    return None


def main():
    try:
        call = json.load(sys.stdin)
    except ValueError:
        return 0
    try:
        problem = problem_for(call)
    except Exception as error:
        if call.get("tool_name") in FILE_TOOLS | {SHELL_TOOL}:
            print(f"write guard failed ({error}); refusing to be safe", file=sys.stderr)
            return BLOCK
        return 0
    if problem:
        print(problem, file=sys.stderr)
        return BLOCK
    return 0


if __name__ == "__main__":
    sys.exit(main())

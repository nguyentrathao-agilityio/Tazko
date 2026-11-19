---
name: verify-milestone
description: Check a finished Tazko step or milestone before its pull request - run it on the local target, compare its screens with the prototype, run the review steps and record the evidence. Use when a step's code is done, before opening its pull request, or when asked to accept a milestone.
---

# Verify a step or milestone

Work only on the local target (`127.0.0.1`). Never point anything at the shared server.

## 1. Know what is being checked

- Read the step in `docs/progress.md` and the milestone's "Done when" list in `plan.md`.
- Find the milestone's screens in the table in `design/README.md`.
- Write the list of checks down first: every "Done when" line, every screen, and every permission rule the step touched.

## 2. Run it for real

- Build a release binary: `cargo build --release`.
- Use a copy of a data folder seeded with `tazko dev seed`, never the folder you work in, and serve it on a free local port.
- Drive the app with a scripted headless browser from a scratch folder outside the repository, at 1440x900. Use two browsers when the step has live updates.
- For each check, record the result and the evidence: a test name, a screenshot path, a timing, or the `curl` call and status code.
- Repeat the permission checks as each role the step touches: owner, member, observer, someone outside the project, and a token limited to one project when tokens apply.

## 3. Compare with the prototype

- Open each screen in `design/tazko-prototype.html` and in the app, light and dark, at the same size, and save both screenshots side by side.
- Look at layout, wording, icons, states (empty, loading, error, read-only) and keyboard behaviour. Note every difference; fix the ones the plan and prototype settle, and list the rest for the owner.
- A screenshot comparison is for people. Do not add pixel comparisons to the tests; behaviour belongs in tests.

## 4. Review the diff

In this order, fixing what each step confirms before the next:

1. `/simplify`
2. `/code-review` at `high` when the change touches sign-in, permissions, tokens, uploads, webhooks or more than one company, otherwise `medium`
3. `/security-review` when the change touches sign-in, permissions, tokens, uploads or webhooks
4. `scripts/ci.sh`, which must end with `ci: ok` on a machine with every tool from `docs/dev.md`

## 5. Record the evidence

- Add or update the step's row in `docs/progress.md`: what was built, how each check was verified, and what stays open.
- For a milestone, add an acceptance section like the M1 to M3 ones in `docs/progress.md`: one row per "Done when" line with its result and evidence.
- In the pull request, list under "How" what the run, the comparison and each review step found and what was fixed.
- Leave the owner's own checks (real teams, real devices, tags) marked as the owner's.

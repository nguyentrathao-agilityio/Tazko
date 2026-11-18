# Open questions

Questions for the owner that are not answered yet. When one is answered, move the answer into `plan.md` (or the file it concerns) and delete it here.

| # | Raised | Question | Current behaviour until answered |
| --- | --- | --- | --- |
| 1 | Step 1.8 | The prototype's fonts, Hanken Grotesk and DM Mono, are under the SIL Open Font License, which is not on the allowed licence list (MIT, Apache-2.0, BSD, ISC). The prototype loads them from a third-party font service. Options: (a) allow OFL for fonts only and serve them from the binary; (b) load them from the font service, which sends every visitor's address to a third party; (c) keep the system font. | (c): the token font stack is kept, so a machine with the fonts installed uses them; nothing is loaded from the network. |
| 2 | Plan Decisions | Rows still marked Assumed in `plan.md` (outside people, web stack, import and export, command line shape, integration details, voting default, card fronts, uploads, ordering) and the "Assumptions and open items" list. | Built as the plan assumes. |
| 3 | Step 1.6 | Invitations by email: the prototype's "Email the invitation" box needs email (M6). | The box is hidden while email is off; the admin shares the site address by hand. |
| 4 | Step 1.9 | Licence exceptions outside MIT, Apache-2.0, BSD and ISC. Rust: `foldhash` (Zlib, required by `rusqlite`) and `unicode-ident` (Unicode-3.0, used by every derive macro while compiling). Web build tools that never ship: `lightningcss` (MPL-2.0, used by Tailwind 4) and `lru-cache`, `graceful-fs` (BlueOak-1.0.0). Are these acceptable? | Allowed as named exceptions in `deny.toml` and `scripts/checks.py`; any other package outside the list fails CI. |


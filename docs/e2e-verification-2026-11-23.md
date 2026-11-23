# End-to-end verification, 23 Nov 2026

Every flow of the app checked again on `dev` at `d170f98` (PRs up to #55 merged), after M6 was merged. Result: **82 of 82 checks pass**. The app needed no change; `scripts/ci.sh` on `dev` failed once on a web test that raced the card dialog's loading state, and that test and two like it now wait for the content (below).

## How it was run

- Release build of `dev` on a copy of `./data` on `127.0.0.1:8792` (never the real data), with `tazko dev mail` catching email on port 2525, `TAZKO_WEBHOOKS_ALLOW_LOCAL=1` and a local webhook receiver on port 8799. A second copy on port 8793 ran with email off.
- Five people with their own sessions: a super admin who owns Tazko Demo, a project member (also a member of a second company), a project observer, a company member outside the project, and the owner of the second company.
- API checks with a script for every role and permission; a headless browser at 1440×900 in light and dark and at 390×844 for the screens, two browsers for live updates, the mouse for drag and drop, and the keyboard for the palette and zen mode; the release `tazko` program for the command line.
- Each row says what was done and what came back. Rows marked with a milestone repeat that milestone's "Done when" lines in `plan.md`; the others are everyday journeys around them.

## Results

### M1 Platform

| Check | Result | Evidence |
| --- | --- | --- |
| Super admin session reads /me | Pass | GET /me 200, is_super_admin=True |
| Super admin creates a company | Pass | POST /admin/companies 201 |
| Suspended company is closed to its members | Pass | suspend 204; member GET my-projects 404 |
| Resumed company opens again | Pass | resume 204; member GET my-projects 200 |
| Deleting a company moves its folder to trash/ | Pass | DELETE 204; trash=['e2e-temp.1795416465', 'e2e-temp2.1795416475'] |
| A member is refused the admin API (hidden as 404) | Pass | GET /admin/companies as member: 404 |
| A company member is refused the admin API (hidden as 404) | Pass | GET /admin/companies as company member: 404 |
| Admin People lists every person | Pass | GET /admin/people 200 |
| Admin actions are in the audit log | Pass | GET /admin/audit 200; actions include ['company_created', 'company_deleted', 'company_resumed', 'company_suspended', 'invite_created', 'project_created', 'sign_in_refused', 'super_admin_granted'] |
| Someone outside a company gets 404 for it | Pass | other-company person: my-projects 404, board 404 |
| Admin Server page data loads | Pass | GET /admin/server 200; keys ['companies', 'data_bytes', 'email_queue', 'email_server', 'last_backup', 'last_restore_test', 'public_url', 'uptime_secs'] |
| No session means 401 | Pass | GET /me without cookie 401 |
| A person in two companies sees only the chosen company's data | Pass | abc: ['Abc project', 'Abc project']; tazko-demo: ['E2E Lan owned', 'E2E Lan owned', 'Website relaunch'] |
| Ids from one company never open another company's data | Pass | abc owner opening a tazko-demo card 404; Lan opening her abc board 200 |

### M2 Core boards

| Check | Result | Evidence |
| --- | --- | --- |
| Owner adds a member and an observer to a project | Pass | members {'the owner's address': 'owner', 'lan@e2e.test': 'member', 'minh@e2e.test': 'observer'}; PATCH role 204 |
| A company member who is not an owner cannot create a project | Pass | POST /projects as member 403 |
| A project member creates a card | Pass | POST cards 201 id 73 |
| Edit, move, assign, label, checklist and comment a card | Pass | patch 204 move 200 member 204 label 204 checklist 201 item 201 comment 201; card now in list To do, members [2], labels [1], checklist items 1, comments 1 |
| A project observer can read but cannot edit, comment or add cards | Pass | read 200, edit 403, comment 403, create 403 |
| A company member not in a project cannot see it | Pass | board 404, project 404, my-projects 0 |
| A company owner cannot open a project's boards until they add themselves | Pass | create project for Lan 201; board before 404; add self as owner 204; board after 200 |
| A company owner archives and restores a project | Pass | archive 204, restore 204 |
| A restricted board is hidden from project members not added | Pass | create 201; member GET board 404; member board list ['Sprint 12', 'Roadmap', 'E2E ranks'] |
| Moving 200 cards one by one creates no rank collisions | Pass | 200 cards moved; order kept True; duplicate positions 0 |
| Description HTML is sanitised on the server | Pass | PATCH 204; stored '<p>Hi<a>x</a></p>' |
| Theme, zoom and time zone are saved with the person | Pass | PATCH 200 ; now theme=dark zoom=110 tz=Asia/Ho_Chi_Minh |
| Search finds cards only where the person can see them | Pass | member hits ['E2E member card renamed', 'E2E member card renamed', 'E2E member card renamed']; outsider status 200 hits 0 |
| Archive and restore a card | Pass | archive 204, restore 204 |
| A card dragged with the mouse lands in the other list | Pass | dragged 'Footer links and legal pages' from Review to In progress with the mouse; the API then listed it in In progress |
| Z turns on zen mode and Esc leaves it | Pass | zen screenshot shows only the lists and 'Zen mode. Move to the top edge for controls. Press Z or Esc to leave.' |
| An observer sees no Add a card and no comment box | Pass | observer on Sprint 12: 'Add a card' 0, comment box on the card 0 |
| With the card open the comment box is visible without scrolling | Pass | member, card 1 at 1440×900: comment box inside the viewport |

### M3 Live and files

| Check | Result | Evidence |
| --- | --- | --- |
| A small image uploads to a card | Pass | upload pixel.png 201 {'created_at': 1795416582, 'id': 3, 'image': True, 'kind': 'file', 'name': 'pixel.png', 'size': 69, 'type': 'image/png', |
| A text file uploads to a card | Pass | upload notes.txt 201 |
| A file renamed to look like an image is refused | Pass | upload fake.png (text bytes) 415 {'error': 'this file is named .png but it is not an image Tazko can show; upload a PNG, JPEG, GIF or WebP image'} |
| An upload past 10 MB is refused with a clear message | Pass | upload 10 MB + 10 bytes 413 {"error":"files can be up to 10.0 MB; this one is larger"} |
| An observer cannot upload | Pass | observer upload 403 |
| A custom field is added and set on a card | Pass | create field 201, set 200, card fields {'1': 5.0} |
| A reaction is added to a comment | Pass | reaction 200 |
| A mentioned person gets a notification for that card | Pass | owner notifications [('mention', 73), ('due', 7), ('due', 6)], unread 3 |
| Watching a board notifies about new cards on it | Pass | watch 200; Lan's notifications ['card_added'] |
| A board background image uploads | Pass | background upload 200 {'background': 'img:1'} |
| Company storage use is reported | Pass | GET storage 200 {'limit': 5368709120, 'max_file': 10485760, 'used': 4529813} |
| Two browsers on one board see each other's changes within a second | Pass | Lan created a card: seen in the owner's browser after 208 ms; Lan moved it to Done: seen in Done after 165 ms |
| A dropped connection shows missed changes after it comes back | Pass | owner's browser set offline, Lan added a card, back online: card shown 181 ms later (the browser may keep the socket open, so this is weak evidence; the reconnect path also has a web test) |
| A mentioned person opens the card from the bell | Pass | bell → 'mentioned you' item → /c/tazko-demo/b/1/c/73 with the card dialog open |

### M4 Views and templates

| Check | Result | Evidence |
| --- | --- | --- |
| A super admin saves a board as a template and offers it to every company | Pass | save 201; offer 201 {'id': 'a:1'}; visible in the other company [('a:1', 'E2E sprint template')] |
| A template creates an identical board in another company | Pass | create from template 201; lists equal True; labels equal True; cards 20 vs 20 |
| A card made into a template creates cards with its checklists and labels, and stops being offered when unmarked | Pass | mark 204; offered True; use 201; labels [1] vs [1]; checklists 1 vs 1; unmark 204; still offered False |
| My cards lists only projects the person belongs to | Pass | Lan's cards 3, projects {'Website relaunch'} |
| My cards with All companies includes the person's other companies | Pass | companies in All companies {'tazko-demo', 'abc'} |
| Board data the dashboard counts from is complete | Pass | Sprint 12 has 21 open cards in 5 lists (dashboard compared in the browser pass) |
| Ctrl K opens the palette; typing narrows it; Enter runs the item | Pass | 25 items → typing 'dashboard' left 1 → Enter opened /b/1/dashboard |
| A card action with no card selected says so | Pass | 'archive the selected' → toast 'Select a card first: press J or K on the board, or open a card' |
| Sorting and grouping in the table never change card order on the board | Pass | sorted by card twice and grouped by list: board list/card ids identical before and after |
| Dashboard totals equal the board | Pass | board: 38 open cards, 6 done, lists 23/5/3/2/5; dashboard: Cards 38, Complete 6, per list 23/5/3/2/5 |

### M5 Card depth

| Check | Result | Evidence |
| --- | --- | --- |
| A checklist item with an assignee appears in that person's My cards with its due date | Pass | PATCH item 204; Lan's items [('Export the page list', 1795675883)] |
| An observer cannot assign checklist items | Pass | observer PATCH item 403 |
| Completing a weekly card moves its due date a week and unticks its checklist; undo restores both | Pass | complete 200: due +7 days, done=False, items ticked [False]; undo 204: due back True, items [True] |
| A link shows on both cards; completing the blocking card clears Blocked | Pass | link 204; blocker sees ['blocks']; Blocked before True, after done False |
| Removing a link removes it from both cards | Pass | DELETE link 204; links left 0 and 0 |
| Voting: off until the project turns it on; a second vote removes it; observers cannot vote | Pass | vote while off 400; turn on 204; vote 204 → voters 1, tile 1; unvote 204 → 0; observer 403 |
| With aging on a quiet card is quiet for its days and a comment restores it | Pass | aging on 204 (7 days); quiet card last change 20 days ago, after a comment 0 days (fading itself checked in the browser pass) |

### M6 Integration and email

| Check | Result | Evidence |
| --- | --- | --- |
| Personal tokens: read cannot write, a project-limited token stays in its project, tokens cannot manage tokens | Pass | read token: board 200, create card 403; write token for Website relaunch: create card 201, other project 404, other company 404; token listing tokens 403 |
| A token over its rate limit gets 429 | Pass | 125 calls: 117 × 200, 8 × 429 |
| Company tokens: owner-only, limited to chosen projects, kept off personal routes | Pass | create 201; board 200; other project 404; /me/cards 403; member creating one 403 |
| A revoked token stops working at once | Pass | revoke 204; next call 401 |
| Webhook addresses to private, link-local, mapped or credential URLs are refused | Pass | {'https://10.1.2.3/x': 400, 'https://169.254.169.254/latest': 400, 'https://[::ffff:10.0.0.1]/': 400, 'https://user:pw@example.com/': 400} |
| Members cannot add webhooks | Pass | member POST webhooks 403 |
| Webhook deliveries carry a signature that verifies with the secret | Pass | 2 deliveries, all verify: True |
| A failing endpoint is logged and scheduled for a retry | Pass | latest delivery [('pending', 500, 1)], next try in 60 s |
| A paused webhook gets nothing | Pass | pause 204; deliveries to it while paused 0 |
| A paused webhook refuses the test event | Pass | test while paused 400 |
| With email on an invitation is emailed | Pass | invite 201 emailed=True; subjects ['You are invited to Tazko Demo on Tazko'] |
| Members are kept out of Integrations and the admin console in the browser | Pass | member: Integrations says it is for owners; /admin says 'Only super admins can open the admin console.' |
| tazko auth login signs in through the browser and tazko card create works | Pass | fresh config folder: code GSJC-HXLT typed at /cli/authorize as Lan, Authorize; token file 0600; card create --label Frontend made 1/294, card move → Review, card done, my cards, search, board list --project 'Website relaunch'; no token printed |
| With email off nothing breaks and the invite shows the address to share | Pass | server on 8793 without SMTP ('email off'): invite with send_email answered 201, emailed=false, share_url=http://localhost:8793 |
| Someone who turns off mention emails still sees mentions in the app | Pass | emails for the second mention 0; bell ['mention', 'mention', 'mention'] |
| A mention is emailed with an unsubscribe header | Pass | mention email arrived; List-Unsubscribe present |
| Unsubscribe link: GET shows a page, POST turns emails off | Pass | GET 200, POST 200, email_frequency now never |

### Whole app in the browser

| Check | Result | Evidence |
| --- | --- | --- |
| All 25 app and admin pages load in light and dark at 1440×900 with no console errors and no sideways scroll | Pass | 25 routes × 2 themes (company, projects, project tabs, board, table, dashboard, card, my cards, integrations, company settings, 6 personal settings tabs, 5 admin pages, /cli/authorize): HTTP 200, a heading on each, 0 console or page errors, no horizontal overflow; screenshots in shots/ |
| Key pages at phone width (390×844) have no sideways scroll or errors | Pass | 10 routes (board, card, table, dashboard, my cards, notifications, integrations, admin server, /cli/authorize): horizontal overflow 0 px, 0 errors |

## Notes from the run

- `scripts/ci.sh` on `dev` after the run failed once in `src/card/aging.test.tsx` ("fades quiet cards…"): the test read the card dialog right after it appeared, while it still said Loading…, which only shows when the machine is busy (five runs on their own passed). It now waits for the text with `findByText`; two tests in `src/card/card.test.tsx` that read the card dialog the same way now wait too.
- The first mention-email check reported nothing sent: the browser pass had opened that mention from the bell first, and read notifications are never emailed (by design, step 6.4). Run again without reading it, the email arrived with `List-Unsubscribe`, and the unsubscribe page and button worked.
- Admin routes answer 404 rather than 403 to people who are not super admins, so the console is not advertised.
- At phone width the card dialog's title column is narrow, so a long title breaks inside words ("navigati-on"). The phone layout was step 2.10, proposed and not built; listed for the owner, not counted as a failure.
- The Templates page button that was cut off for super admins in the M4 acceptance now fits ("Offer to every company" and "Stop offering").
- Checking the reconnect: setting the browser offline may leave its socket open, so the 181 ms result is weak on its own; the reconnect path also has a web test.

## Not checked here (owner or not built)

- Through the real tunnel and shared server: the site loading with no new port, rollback of a broken deploy, the nightly backup and weekly restore test on the server, real Google sign-in of a Workspace and a Gmail account (including a refused account appearing in the audit log, which the log here shows from earlier runs), inviting a new email to a project and signing in.
- A second computer running the install script and `tazko auth login`; real SMTP; a real webhook receiver.
- Import and export of a board file (step 2.9) and undo across sessions, card numbers and the phone layout (step 2.10): proposed, not built.
- A real team using a sprint board for a while (M2) and two people on one board for a day (M3).

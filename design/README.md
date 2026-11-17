# Design

The clickable prototype is the specification for how Tazko looks and behaves. Version 2 is the reference for M1 to M6, and a milestone is not finished until its screens match.

| File | What it is |
|------|------------|
| `tazko-prototype.html` | Prototype v2 in one file. Open it in a browser; no server needed. |
| `tokens.css` | Colours, type, shadows, radii and sizes for light and dark, to become the Tailwind theme. |
| `CHANGELOG.md` | What changed in each prototype version. |

This folder is read-only for the build. Ask the owner to change it.

## Using the prototype

- Start from the index page. Each tile opens a screen; the "v2" group at the bottom opens each change made in v2.
- The **View as** switch in the dark bar shows what each project role sees (owner, member, observer). Build the same differences, enforced on the server.
- Before a milestone, open its screens below and note layout, states (empty, loading, error, read-only), wording, icons and keyboard behaviour.
- Recreate each screen with real components. Do not copy the prototype's scripts; its CSS values, icon shapes and wording may be reused.
- The prototype only pretends: sample data, live updates, email, uploads, the command line, undo across sessions. Build those as `plan.md` describes.
- Never build the prototype-only controls: the dark prototype bar, View as, Live sim.
- Where the prototype and `plan.md` differ on data, permissions or limits, the plan wins; on look or interaction, the prototype wins.

## Screen index by milestone

| Milestone | Prototype v2 screens |
|-----------|----------------------|
| M1 Platform | Sign in, Sign-in refused, Company home, Company settings, Manage projects, Admin: Companies, Company detail, People, Invites, Audit log, Server |
| M2 Core boards | Your projects, Project boards, Project members, Project settings, Board, Card, Filter, Search, Zen mode, Long cards, Rich text, Import a board, Board menu → Export board, Settings: profile and appearance |
| M3 Live and files | Attachments (10 MB), Custom fields, Live updates, Notifications, Reactions, Board background upload |
| M4 Views and templates | Templates, Card menu → Make template, Table view, Dashboard, My cards (All companies), Command palette |
| M5 Card depth | Checklist items, Recurring cards, Linked cards, Voting, Card aging |
| M6 Integration and email | Settings: notifications, access tokens and command line, Authorize CLI, Integrations: API tokens and webhooks, Email previews, Admin: email status |

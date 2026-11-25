# Import and export

## Export a board

Board menu → **Export board** downloads `tazko-<board>.json`. Anyone who can open the board can export it; a restricted board is exported only by the people on it. API: `GET /api/v1/c/<company>/boards/<board>/export`.

The file is Tazko's own board export, `"format": "tazko-board-export"`, `"version": 1`:

| Field | What it holds |
| --- | --- |
| `exported_at` | Unix seconds |
| `board` | `name`, `description` (plain text, as in About this board), `background` |
| `labels` | `key`, `name`, `color` (0 to 7) |
| `people` | `key`, `name`, `email` for everyone the cards mention |
| `lists` | `title`, `archived`, `cards` in board order |
| card | `title`, `description_html`, `labels` and `members` (keys), `start_at`, `due_at`, `done`, `archived`, `checklists` (`title`, `items`: `text`, `done`, `assignee`, `due_at`), `comments` (`author`, `author_name`, `text`, `created_at`), `links` (`name`, `url`), `files` (`name` only) |

Not in the file: uploaded files themselves (listed by name), custom fields, votes, links between cards, reactions, activity, and board access.

## Import a board

Project boards → **Import a board**, for people who can create boards in the project (owners, and members when the project lets members create boards). It reads:

- Tazko's own export file (above), version 1.
- A generic board export file: an object with `name`, `desc`, `labels` (`id`, `name`, `color`), `lists` (`id`, `name`, `closed`, `pos`), `cards` (`id`, `name`, `desc` in Markdown, `idList`, `idLabels`, `idMembers`, `start`, `due`, `dueComplete`, `closed`, `pos`, `attachments` with `name`, `url`, `isUpload`), `checklists` (`idCard`, `name`, `pos`, `checkItems` with `name`, `state`, `pos`, `idMember`, `due`), `members` (`id`, `fullName`, `username`, optional `email`) and `actions` (comments are actions of type `commentCard` with `data.text`, `data.card.id`, `idMemberCreator`, `memberCreator`, `date`). Dates are ISO 8601. Label colour names are mapped to the nearest of Tazko's 8 colours.

Steps: choose the file (up to 50 MB); check the counts and match people; import.

- **People** are matched with the project's members by email, then by name. Each can be changed or left out. Card members and checklist item assignees need a project member who can edit. Every imported comment is marked *imported* under its time, keeps its original date, and shows its author: the matched member, or the name from the file. Nobody is invited or added to the project by an import.
- **Options**: bring archived cards (they land archived) and archived lists (cards in them keep their own state, as when a list is archived in Tazko), comments and link attachments. Uploaded files in the old tool are not copied.
- The new board is open to the whole project. The import is recorded in the board activity and the company's `imports` table (file name, counts, people matches). The server audit log, which super admins read, gets the project name, board id and counts only, never the board name.
- API: `POST /api/v1/c/<company>/projects/<project>/imports/preview?name=<file>` with the file as the body returns the counts, lists, people with suggested matches and the project's members, and saves nothing. `POST /api/v1/c/<company>/projects/<project>/imports` imports it; the body is a JSON object `{ "name": <file name>, "archived": true, "comments": true, "links": true, "people": { <file person key>: <member id or null> }, "file": <the export file> }`, where `people` lists only the choices that differ from the suggestions.

### Limits and safety

- Files over 50 MB are refused before parsing, and at most two files are parsed at a time. Lists in the file are read with fixed caps (for example 20,000 cards, 100 labels, members, checklists and links per card, 1,000 items per checklist, 5,000 comments per card), so an oversized file stops while it is being read. A board can bring up to 1,000 lists, 20,000 cards, 500 labels, 5,000 people, 50,000 checklists, 200,000 checklist items, 200,000 comments and 100,000 links.
- Titles and names lose control characters and are cut to the app's limits; descriptions are cleaned with the same allow-list as the editor (Markdown is converted first); comments are plain text up to 10,000 characters; only `http` and `https` links are kept; dates outside the years 1970 to 3000 are dropped.
- Nothing is written until the whole file has been read and checked; the board is created in one transaction.

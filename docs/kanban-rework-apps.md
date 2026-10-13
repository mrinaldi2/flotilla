# Kanban rework: desktop, phone and linked computers

Status: **proposal, awaiting owner approval**. This is the companion to
[kanban-rework.md](kanban-rework.md), and the terms used here are defined
there.

The design reuses what exists rather than adding new chrome:

- the project window header and its pills;
- the Board tab;
- the item drawer;
- Needs you, which receives proposals as decisions.

Words follow the glossary rule: a glyph and a word, never colour alone.

## Desktop

### Board tab with several boards

The Board tab keeps its place (⌘2). A board switcher sits at the start of the
board toolbar. The selection gains `board?: string`, so links and Needs you
rows open the right board.

```
┌ Projects › Recipe Box   ● 3 bots   ▲ 2 need you   ◐ No release yet ───────┐
│ Overview  Board  PRs  Team  Releases  Meetings                    More ▾  │
├──────────────────────────────────────────────────────────────────────────┤
│ [Main ▾]  Filters: bots · type · labels       + New item     ⚙ Board     │
│ ┌ To do  4 ┐ ┌ Doing  2/3 ┐ ┌ QA  1 ┐ ┌ Done  12 ┐  + Column              │
│ │ ▲ RB-14  │ │ ✱ RB-9     │ │ ...   │ │ ...      │                         │
│ │ ...      │ │ ...        │ │       │ │          │                         │
└──────────────────────────────────────────────────────────────────────────┘
```

- **Switcher (`Main ▾`):** lists the boards, the default first, then the
  others by `ord`, then "Archived boards…" and "New board…". It remembers the
  last board per project, as the board's filter preferences do today.
- **Board deep links:** a Needs you row or an item link opens the item's own
  board.
- **Empty project:** the project shows "No board yet" and offers "New board".
  - With `approve`, the owner picks a preset.
  - A line explains that the lead can also propose a board.

### New board

A sheet with three fields:

- name;
- key, suggested from the name and checked for uniqueness as you type;
- preset (Simple / Software / Release-gated), each shown with a preview of its
  columns.

Create opens the board.

### Board settings (⚙ Board)

A right-side panel, the same width as the item drawer, with four sections:

| Section | Contents |
|---|---|
| General | Name, key (read-only once items exist), description, default board, structure policy (`owner_confirms` / `lead_free`), archive |
| Columns | One row per column: drag handle, name, kind chip (`○ open` / `◐ active` / `● done` / `⊘ cancelled`), WIP, visible. Expanding a row shows who may move items in, allowed-from, and the entry rules as a checklist with parameters. |
| Item types | Glyph, name, template sections. Which fields are on (size, priority, acceptance criteria). Label groups. |
| Automations | One row per fact: `When  PR merged  →  move to  Done`. The facts list comes from what the modules registered. |

- **Every save is a board change.** The panel footer shows "Last change: by
  Lead, 3 min ago · History".
- **Removing a column that holds items** asks where to move them first. The
  move is recorded per item.

### On the board itself

- **Adding and editing columns:** "+ Column" at the end adds a column inline
  (name, then kind). The column header menu offers rename, WIP limit, rules…,
  hide and remove.
- **Creating items:** "+ New item" (toolbar, and at the foot of every `open`
  column) opens a quick add: type, title, then Enter. The item drawer opens on
  the new item.
- **The item drawer becomes editable:** title, description (markdown with the
  template sections), acceptance criteria, assignee, priority, size, labels and
  links. Each control appears only when the board has that field turned on.
- **Refusals:** shown as today (`RefusalPopover`), now listing the column's
  rules by name.

### Changes made by the lead

- **Notice strip:** a change the owner did not make shows a strip at the top
  of the board until it is dismissed:

  ```
  ↻ Lead added column "QA" and a rule to Review · 2 min ago     View · Undo · ✕
  ```

- **Proposals:** under `owner_confirms`, a proposal is a decision in Needs
  you. The decision card shows a before/after column strip and the rule diff.
  Approving it applies the change; "Change it" opens the settings panel
  prefilled with the proposal.

### Release chip (no change in 0.19.0)

The header's release pill stays where it is. The next plan, single active
release, makes it the always-visible current release.

## Phone (iOS app)

The phone keeps its layout: the project screen's segmented control, Overview ·
Board · Team · Releases · Meetings. The board grows from read-only to useful on
the move. Structure editing stays on the desktop, apart from approving
proposals.

```
┌──────────────────────────────┐
│ ‹ Recipe Box      Main ▾   ⋯ │
│ [Overview|Board|Team|Rel|Mtg]│
│ (To do 4)(Doing 2/3)(QA 1)(…)│
│ ┌──────────────────────────┐ │
│ │ ▲ RB-14 Import recipes   │ │
│ │   Ana · 2/3 AC          ›│ │
│ ├──────────────────────────┤ │
│ │ ✱ RB-9  Fix sort order   │ │
│ │   Blocked                ›│ │
│ └──────────────────────────┘ │
│                     ( + )    │
└──────────────────────────────┘
```

- **Board picker:** a menu in the navigation title (`Main ▾`) lists the
  project's boards.
- **Column chips:** the chips and the paged list per column stay as today,
  built from the selected board's columns.
- **Live sync:** the board watches for changes instead of loading once, so
  moves made elsewhere appear without pull-to-refresh. It falls back to
  reload-on-reconnect.
- **Move:** swipe or the context menu offers "Move to…". It lists the columns
  with their refusals already checked, the same check as the desktop's move
  menu. A reason or override field appears only when the rule needs one.
  Needs `control`.
- **Quick add (+):** type, title and board column, which defaults to the first
  `open` column. Needs `control`.
- **Item view:** keeps comments, adds assignee and labels as read-only rows,
  and keeps the acceptance criteria read-only.
- **Proposals:** board-structure proposals arrive in Needs you like other
  decisions, with the before/after column strip. Approving needs `approve` and
  Face ID, like release rulings today.
- **Contract:** the vendored `kanban.v1` protos and an advertised
  `contracts["kanban"] = 1`. Against a 0.18 daemon, the app falls back to the
  `board.v1` read path it uses today.
- **Design system:** uses the existing Gravity Night components (`Pill`,
  `ItemRow`, `StatusLabel`, `.insetGrouped` lists, `ContentUnavailableView`
  for the empty board).

## Linked computers

Authority stays where it is today: **the project's home computer** holds every
board of the project and decides every move. The rework adds board ids to the
existing paths; the trust model is unchanged.

```mermaid
sequenceDiagram
  participant LB as Bot on linked computer
  participant LD as Linked daemon
  participant HD as Home daemon
  LB->>LD: item_move(RB-14, QA)
  LD->>HD: board_call{project, board, bot, tool, args}
  HD->>HD: run as the bot's stand-in: roles, rules, policy
  HD-->>LD: result, or the unmet rules
  HD-->>LD: board_event{board_id, seq, change}
  LD-->>LB: result
  LD->>LD: update the board's mirror; push to local clients
```

- **One mirror per board.** The linked daemon keeps a mirror per `board_id`
  and a mirrored board list. `board_snapshot` and `board_event` carry
  `board_id`. Structure events (`BOARD_*`, `COLUMNS_CHANGED`) refetch only that
  board.
- **Bots on linked computers** see the same tools. Tool listing uses the roles
  the home gives their stand-in, as now. Structure tools follow the board's
  structure policy on the home, so a linked lead's proposal becomes a decision
  on the home.
- **Notices** to a linked bot (a board changed, an item was moved) travel as
  ordinary bus messages over the peer link and use the existing delivery and
  retry path.
- **Home offline:** every board stays readable from its mirror. Writes are
  refused with "The board lives on <home>, which is unreachable". Mirrors still
  don't survive a restart; `board_home` still records which computer is home,
  so card guardrails keep applying.
- **One home per linked team:** the current rule stays, now phrased per
  project: two linked projects can't both hold boards.
- **Mixed versions:**
  - A 0.18 peer calling without `board_id` is served the default board.
  - A 0.19 peer asking a 0.18 home falls back to the single-board frames.
  - The peer hello advertises the kanban contract version.

## Accessibility and states

- Every new control is reachable by keyboard, and the switcher and settings
  panel follow the existing WAI-ARIA patterns (tablist, menu, dialog).
- Kind chips, rule results and notice strips pair a glyph with words.
- Empty, loading, offline (read-only) and refused states get Ladle stories and
  visual baselines on desktop, and previews on iOS.

# Ticket flow on a fresh project: investigation of the owner's 0.19.0 test

Build: release-0.19.0-kanban-rework at `5c7f2543`. Code was read from
`gravity-wt-0.19.0-kanban`, and every
`file:line` below points into that tree. The data comes from a `.backup` copy of
`~/.thehermes/bus.sqlite`, which was deleted afterwards. I also read
the bot's transcript
(the Claude Code session log of the `projecttest` New Bot)
and `~/.thehermes/logs/hermesd.err.log`. Times are UTC. The owner's clock is EEST (UTC+3), so 17:13Z is 20:13 local.

## TL;DR

The board that was created automatically is the full **release-gated** Main
(H-017). The project's first bot is **not the lead and holds no board role**.
Those two defaults together explain most of what the owner saw:

- **The bot could not touch the card.** It could not attach to it or move it. The
  only moves the release-gated rules would ever let it make need a delegated
  task linked to the card and, for a feature, a branch.
- **Assigning tells the bot nothing.** The owner's later edits to the card do send a
  generic note: "The owner changed P2-001, which is assigned to you: edited it."
  The bot did the work on the first of those notes, at the size edit, while the
  card sat in Inbox. Nothing in its prompt or in the note says what to do with
  the card.
- **The board shows almost none of this.** Inbox is a folded 48px rail on a
  release-gated board. Cards carry no status line, refusals are never stored, and
  board pushes don't say who acted. Toasts exist, but only for the owner's own
  moves.

## 1. Timeline (reconstructed)

| UTC | Source | What happened |
|---|---|---|
| 17:13:49.98 | `project` | Project **ProjectTest** created (`ef5a9d78…`). `lead_bot_id` stays NULL. |
| 17:13:50.02 | `bot` | **New Bot** created (`ffa80374…`, workspace `…/projecttest/bots/new-bot/workspace`). No `project_role` row at any time. |
| 17:13:50.31 | `message` 5572 | System note "You have just been created with no charter… first bot…". Delivered 17:13:52. |
| 17:13:50.42 | `board`, `board_settings`, `template` | Board **Main** created automatically: key `P2`, `flow=release_gated`, `structure_policy=owner_confirms`, no automations. The 9 H-017 columns are Inbox, Ready, Doing (WIP 1 per assignee), Review (3), Verify (3), Awaiting owner (5), Deploying, Done, Cancelled (hidden). Every column has `movers=NULL`, `rules=[]` and `allowed_from=[]`, which is normal: release-gated boards use `guards.rs`. The 5 default item templates came with it, and the feature template's DoR is `acceptance_criteria, size, spec_link`. |
| 17:13:58 | msg 5573 | Bot greets the owner with `asks:true`. The Needs-you row `owner_question fb82d859` is **still open**. |
| 17:14:01.43 | `item_event` 6601 | Owner creates **P2-001 "Test"** (feature) as a **draft**. |
| 17:14:13.34 | events 6602–6603, msg 5574 | Owner invites New Bot to discuss the draft and a `thread` link is added. The kickoff says, among other things, "Ready needs: acceptance_criteria, size, spec_link". |
| 17:14:17 | transcript | Bot calls `message_owner(item:"P2-001", asks:true)` and is refused: "P2-001 is a draft: … only item_get, item_update … work on it". It resends without the item (msg 5575). That Needs-you row (`1b096d27`) is **still open** too. |
| 17:14:26.36 | event 6604, msg 5576 | Owner ends the discussion (uninvites the bot). |
| 17:37:55 | events 6605–6607 | Owner fills in the description, label `web` and 4 acceptance criteria. The "Design/spec link" section is still empty. |
| 17:38:05.69 | events 6608–6609 | **Finalize.** State goes draft → live, moved to `inbox` with the note "finalized from a draft". No assignee yet. Nobody is told: the bot is no longer invited. |
| 17:38:19.38 | event 6610 | Owner **assigns** New Bot. **No message is sent** (no `message`/`delivery` row at 17:38). |
| *(17:38–17:39, inferred)* | owner's report | The owner opens "Move to…" (the 3 dots). Ready is ✗ with "Size it." / "Link the design or spec." Doing is ✓ (it only needs a reason). No refusal is stored anywhere: no table holds one, and the log has no line. |
| 17:39:16.36 | event 6611, msg 5577 | Owner sets **size S**. The assignee notice "[card P2-001] The owner changed P2-001, which is assigned to you: edited it. Read it again with item_get before you go on." is delivered at 17:39:16.93. The bot goes `working` (log: "prompt submitted"). |
| 17:39:20 | transcript | `item_get`: card is in `inbox`, Design/spec link still empty. |
| 17:39:48 | transcript | Bot writes `artifacts/P2-001-button-variants.html`, which is the whole task. |
| 17:39:51–52 | transcript | `item_attach` is **refused**: "item_attach needs a board role you don't have, or an item assigned to you or a task linked to it; ask the lead". The card *is* assigned to it. `item_move_check` returns every column as not allowed: Ready needs "Only the lead or the owner" plus `dor.spec_link`; Doing, Review, Verify and Awaiting owner are "Only the owner…" plus `reason.required`; Deploying and Done are `move.daemon_only`; Cancelled is lead/owner only. **The bot never calls `item_move`.** |
| 17:39:57.13 | event 6612 | Bot comments on the card: "Done: the page is at artifacts/…. … I couldn't attach the file here or move the card, because I don't have the board permission for either." |
| 17:40:00 | msg 5578 | Bot sends `message_owner`: "P2-001 is built … moving it on is up to you." |
| 17:40:08.42 | event 6613 | Owner fills in "Design/spec link: No specific design is needed", which **satisfies the DoR**. The assignee notice is **folded**, because it is within 60 s of the 17:39:16 one. No message is sent. |
| 17:40:08.62 | log | Bot `ready` ("turn complete"). |
| 17:40:20.75 | event 6614, msg 5579 | Owner moves **Inbox → Ready**, which is allowed: owner, DoR met. The notice "…: moved it to ready…" is sent (64 s after the last one, so not folded). |
| 17:40:20–26 | transcript | The bot reads the card, sees the work is done, and does nothing. Its words: "Moving the card to Doing or any later column is owner-only, so I'm leaving it where it is." |
| 17:43:28 | transcript | Away summary: card "built and on Ready… only they can [move it]". |
| now | `item` | P2-001 is in `ready`, version 7, assigned to New Bot. No facts, no tasks, no `artifact` link, all 4 AC unchecked, no decisions. |

## 2. The four points

### Point 1: finalize → Inbox, assignment ignored, Ready refused but Doing allowed

**Why finalize lands in Inbox, and whether that is right.** `finalize` puts a
draft live in the board's **first open column** unless the owner picks another
(`crates/hermesd/src/kanban/drafts.rs:147-199`; the default is `open` at :175-183, `go_live` at :192).
The draft footer has a Column picker that defaults to the first open column
(`apps/desktop/src/components/board/drafts/DraftParts.tsx:101-104, 112-142`). On a release-gated board Inbox
means "not refined yet". It does not mean "someone's inbox", so an unassigned
item there is correct H-017 behaviour (`kanban/guards/conditions.rs:203-266`
is the Inbox→Ready DoR). The word "Inbox" invites the reading the owner gave
it, though. Whether a card is assigned has nothing to do with this column; the
assignee only matters at Doing. A draft cannot be assigned before finalize
(`kanban/edit/more.rs:1`: assigning needs a live item), so "finalize, then
assign" is the only path. **Verdict:** working as designed, but the default and
the column name are bad. On a Simple preset the column would be "To do".

**Why assigning didn't make the bot take it.** The owner's assign sends
**no notice**. In `ws/board/item_edits.rs:60` `ItemAssign` maps to `what = ""`,
and `:78` only tells the assignee when `what` is non-empty. The spec says an
assignee is told when the owner or another bot moves or edits their item
(`docs/kanban-rework.md:287-291`). Becoming the assignee was left out, and the
only caller of `tell_assignee` is the owner's WebSocket path (`ws/board/item_edits.rs:79, 91, 125`),
so a bot's `item_assign` or `item_move` tells nobody either. The bot started
only when the *next* owner edit (size, 17:39:16) produced the generic "edited
it" note (`kanban/notices.rs:40-60`, text at :50-54). **Verdict: bug.** A gap
against the K4b spec, plus missing notices for changes bots make.

**Why Inbox→Ready is refused while Inbox→Doing is allowed.** It is the H-017
release-gated table, on the board that was auto-created:

- The board is release-gated because `db/board.rs:83-155` (`ensure_board`) always
  inserts Main with `defaults::COLUMNS`, and the `board.flow` column defaults to
  `release_gated`. It is called from `ws/board/reads.rs:22-34`: the first
  `board_get` on a project with no board enables it on its own (`Enable::Auto`, :52-77).
  This is the known gap G1 (`docs/kanban-rework.md:412`, `docs/kanban-k9-runbook.md:159, 215`).
- Inbox→Ready is `Rule::Refine` (`kanban/guards.rs:174`). The owner passes the
  actor check (`guards.rs:269-272`) but not the conditions: DoR runs for
  everyone (`guards/conditions.rs:31`, `definition_of_ready` :203-266). At that time
  `size` was missing (until 17:39:16) and `spec_link` was missing until 17:40:08
  (the check is at :217-222: an `artifact` link or text under "Design/spec link").
- Inbox→Doing has no row, so it is `Rule::Unlisted` (`guards.rs:184`). Only the owner
  may make it (`guards.rs:294`), and only with a reason (`conditions.rs:30`). The
  desktop treats a refusal that needs nothing but a reason as allowed
  (`apps/desktop/src/components/board/moves.ts:22-35`, so `planMove` returns `input`), and
  the Move menu marks Doing ✓ (`MovePopovers.tsx:150-170`).

**Is this a guard bug?** Not by the letter of H-017: "anything else, the owner
with a reason" is the owner's documented escape hatch (`guards.rs:163-164`).
It is inconsistent, though. Through the escape hatch the owner skips the DoR
*and* the Start conditions (assignee set, not an epic, a **delegated task
linked**; `conditions.rs:32-54`). Through the proper path Inbox→Ready→Doing,
the owner would have been refused at Ready (DoR), and then at Doing for "No delegated task is
linked" (`conditions.rs:47-53`). So the short cut is easier than the road, and
the ✓ in the menu makes it look like the intended move. **Verdict:** a bad
default (the release-gated board on a new project), plus a UX and rules
smell. The fix I propose is to stop putting new projects on release-gated
(fix A). If release-gated stays reachable, also show "needs a reason" as its own mark rather than ✓.

**Also found here: the first bot is not the lead and has no role.**
`ensure_board` gives the Lead role only to `project.lead_bot_id`
(`db/board.rs:144-151`), and nothing sets that for a project's first bot (it is set only from the
UI, `ws/decisions_publish.rs:153-166`). With no role the bot is not a "Member"
(`mcp/board_schema.rs:19-23`), so `item_attach`, `item_update`, `item_create` and
`item_block` are refused (`mcp/board.rs:72-79`). Being the assignee only unlocks
`item_move`, `item_move_check`, `item_link` and `item_comment` (`mcp/board_drafts.rs:20-21`). The
refusal text says "…or an item assigned to you…", and that is **wrong** for
`item_attach`, because the item *was* assigned to the bot. **Verdict: bug**,
both in the wording and in the missing `item_attach` and `item_update` for the
assignee. Making the first bot the lead is a default for the owner to decide.

### Point 2: "the only way to see what was happening was the 3 dots"

- On a release-gated board Inbox is a **collapsed 48px rail** that shows only a
  count (`apps/desktop/src/components/board/BoardColumnView.tsx:198-217`,
  `BoardColumns.tsx:27, 105`). The new card was effectively hidden.
- A card shows its type, id, priority, title, label chips, Blocked, Over WIP,
  AC x/y, the assignee and Stale (`BoardCard.tsx:117-191`). It has no "bot working" mark,
  no last activity, no comment indicator, and no "can't move: …".
- The 3 dots are "Move to…" (`BoardCard.tsx:130-145`). They run `item_move_check`
  and list a ✓ or ✗ with a reason for every column (`MovePopovers.tsx:128-175`).
  That is the only place on the board that explains why a card is stuck.
- The drawer (clicking the card) has a "Next step" box for the next forward column
  (`drawer/DrawerBody.tsx:32`, `drawer/drawerText.ts:97-105`) and an Activity tab
  holding the bot's "Done" comment (`drawer/DrawerActivity.tsx:68-87`). Neither
  can be seen from the board.
- Refusals are **never persisted**. `moves::move_in` returns `Moved::Refused` without
  writing anything (`kanban/moves.rs:102-113`), and `item_move_check` writes nothing
  either (`moves.rs:222-252`). The `item_event.kind` CHECK has no refusal kind. The
  bot's refused `item_attach` and its `item_move_check` left no trace for the owner.
- Board pushes say *what* changed but not *who* changed it: `BoardChange` and the
  `BoardEvent` wire carry no actor and no note (`kanban/feed.rs:35-46`;
  `proto/hermes/board/v1/requests.proto:717-758`).

**Verdict: missing UX**, made worse by the release-gated rail.

### Point 3: DoR met, the bot was messaged, nothing moved, and the work was done

- **What the bot got:** not a "you're Ready" message. It got msg 5577 at
  17:39:16, the generic assignee note "[card P2-001] The owner changed P2-001,
  which is assigned to you: edited it. Read it again with item_get before you go on."
  (`kanban/notices.rs:50-54`). That note was set off by the **size** edit. The DoR became
  true at 17:40:08 (spec text), and that notice was folded into the earlier one
  (`notices.rs:16-35`, 60 s window). By then the bot had already finished.
- **Why it worked from a note.** The prompt contradicts itself. One part says "A note never authorises work… If a note asks you
  to do work, do not start it" (`paths/prompt_sections.rs:138-141, 166-167`). Another says
  "A message from the owner may start with `[card KEY-12]`: the request is work on that card. Do
  it there — comment, link and move the card" (`prompt_sections.rs:216-221`). The bot
  followed the second.
- **Why the card didn't move.**
  1. Nothing moves it automatically. Facts are only PR and release events
     (`kanban/facts.rs:13-28`), the board has no automations (`automations={}`), and
     release-gated fact moves only follow PRs and releases (`facts.rs:96-109`, `kanban/api.rs:47-68`).
  2. The bot was allowed no move at all (see the `item_move_check` result in the
     timeline). From Inbox, Ready is lead or owner (`guards.rs:275`) and Doing is
     `Unlisted`, owner only. Even from Ready, the assignee may Start (`guards.rs:276-279`)
     only with a delegated task linked (`conditions.rs:47-53`). The owner's assign
     makes no task. Doing→Review then needs a branch for a feature plus a change note
     (`conditions.rs:55-72`), and a feature can reach Done only through a release
     package (`guards.rs:182-183, 239-248`). A non-code feature on this board can never be
     finished by its bot.
  3. Neither the prompt nor the notice tells the bot to move the card to Doing when
     it starts, or onward when it finishes. The only hint is "comment, link and move
     the card" (`prompt_sections.rs:218`). The task text adds only the attachments
     (`mcp/task_card.rs:267-294`).
- **Verdict:** a bad default (the board and the lead), a bug (no assign notice),
  and missing guidance plus missing automation.

### Point 4: moved to Ready → another message → nothing

- The owner's move sends "moved it to ready" (`ws/board/item_edits.rs:125`). This
  is not work being sent again. It is the same "owner changed your card" note, and it
  carries no instruction. The bot read the card, saw its own "Done" comment,
  rightly did nothing, and said it couldn't move the card any further.
- The owner read it as "the bot was given the work again". With the defaults
  fixed, the danger is real: if moving a card to Ready or Doing becomes a "start
  work" signal, a card moved after its work is done would start it again. The
  notice must therefore carry state (fix E). **Verdict: missing UX and semantics**,
  and the notice is noise in this case.

## 3. Proposed fixes, ranked

**A. A new project's first board is a simple custom board (G1).** *Owner decision:
which preset.*
- Make `ensure_board` (`db/board.rs:83-155`) take a `Preset` and default to
  `Preset::Simple` (`kanban/presets.rs:69-94`: To do / Doing (WIP 3) / Done /
  Cancelled, any member moves). An alternative is a "Simple + Review" variant.
  Keep the release-gated preset for "New board…" and for existing boards (the
  PhD board and old projects stay as they are).
- `reads.rs:30` (Auto) and `enable.rs:24, 59` both go through `ensure_board`, so it is
  one change point plus its tests. `tests/k9_acceptance.rs` has the G1 workaround and its
  ignored test, which can then be enabled.
- Lower the item templates' DoR for custom boards: a custom board ignores `ready`
  anyway, since only column rules apply (`flow.rs:82-127`).
- Size: S–M (daemon change plus tests, and the desktop's "No board yet" copy).

**B. Make the first bot the project lead, or at least give an assignee what it
needs on its own card.** *Owner decision: whether the first bot becomes lead
automatically.*
- Either set `lead_bot_id` when the first bot is created (`botmgmt/charter.rs`
  already knows it is the first bot, at :45), or
- add `item_attach`, `item_update` and `item_unblock` to `OWN_ITEM_TOOLS`
  (`mcp/board_drafts.rs:20-21`) so an assignee can attach its result and edit its own card.
- Fix the refusal wording in `mcp/board.rs:77-78`, which claims assignment would
  have been enough when it wasn't. Size: S.

**C. Assignment and Ready notices tell the bot exactly what to do with the
card.** *No decision needed for the mechanics. The wording should go through
the Context Engineer.*
- Send a notice on assign: `item_edits.rs:60` becomes `"assigned it to you"`.
  Also send one on unassign, to the previous assignee. Never fold the *assignment*
  notice into an earlier one; the fold in `notices.rs:26-35` would swallow it.
- Build the text from the board instead of the fixed string at `notices.rs:50-54`:
  "[card P2-001] The owner assigned P2-001 “Test” to you. It is in To do. When you
  start, move it to Doing (`item_move`); when it is done, comment the result, link
  the artifact, and move it to Done/Review. `item_move_check` shows what each move needs."
  The columns come from the board, and the text names only moves the bot may make
  (use `item_move_check` results in the composer).
- Send the same notices for bot actors. Call `tell_assignee` from the MCP move and
  edit paths too (`mcp/board.rs`, `mcp/board_edit.rs`), as the spec asks. Skip the
  notice when the actor is the assignee itself.
- Size: S–M.

**D. The bot moves its card when it starts and when it finishes.** *Owner
decision: whether the daemon moves cards on its own.*
- Prompt: add an "Your cards" block to `board_work`
  (`paths/prompt_sections.rs:130-176`):
  - an assigned card is yours to work; move it to the board's first active column
    when you start and onward when you finish;
  - never redo work a card already shows as done;
  - the `[card …]` notices are about your card, not new work.

  Resolve the note-vs-card contradiction (:138-141 against :216-221).
- An automation fact: add `Fact::WorkStarted` (`kanban/facts.rs`, with a name and
  description) reported when an assignee's turn starts with a `[card X]` notice or a
  task on X in flight. A custom board can then map it to Doing in `automations`, and
  the Simple preset ships that mapping. The daemon's move goes through the daemon
  path (`api.rs:73-114`), so movers and WIP don't stop it. A matching
  `Fact::WorkFinished` is too fuzzy, so leave finishing to the bot (prompt) or
  to `pr.merged` on code boards.
- Linking a turn to a card needs one new piece: remember the card of the last
  card-scoped message delivered to the bot. A `message.item_id` column is cleaner
  than parsing `[card …]`.
- Size: prompt S; fact plus the turn-to-card mapping M.

**E. No duplicate work when a card moves after its work is done.** *Owner
decision: what "done" means on a simple board.*
- The move notice says what changed *and* the state: "moved it to Ready. Your last
  comment says it is done; nothing to do unless the owner asks." At minimum: "This is a
  notice, not a request." Build it in `notices.rs` from the card's latest
  assignee comment and its column kind.
- Don't notify for moves the bot can't act on, or that only make the board catch
  up. A move into an open column, for an assignee whose last comment came after the
  last owner edit, can be skipped or folded.
- With fixes A and D in place, the bot would have moved the card to Done itself, and
  the owner's later move would not happen. Size: S.

**F. Refusals and the reason a card is stuck show on the card.** *No decision
needed. The visual design should go to the owner.*
- Card status line: "⛔ Can't move on: Link the design or spec. (+1)". It is
  computed from `item_move_check` for the next forward column, the same thing the
  drawer's Next step shows (`drawerText.ts:97-105`). The board can batch-check visible cards
  or, better, carry a `next_unmet` summary on the `ItemCard`. That would be a new
  optional field in `board.proto:319-348`, filled by `t.card()`.
- The last refused attempt is kept in memory per item (actor, target, unmet,
  time) and shown on the card ("New Bot couldn't move it to Doing: Only the owner…").
  It is pushed as a board event (see §4). Persist it only if the owner wants it in
  the history; that needs a new `item_event.kind`, which means a migration of the
  CHECK constraint.
- Don't fold Inbox into a rail when it holds a card that changed since the owner last
  looked, or drop the rail along with release-gated (fix A).
- Size: M.

## 4. The notifications feature

**Goal:** banners at the top of the board (and of the app when the board isn't
open) that say who did what to which card, each linking to the card.

### What the daemon already has

| Event wanted | Exists? | Where |
|---|---|---|
| Card moved (from → to) | Partly | `BoardEvent{ITEM_MOVED, card, from_column}` (`requests.proto:717-758`), published by the owner (`ws/board/item_edits.rs:115-124`), by bots (`mcp/board.rs:192-…`) and by facts (`api.rs:117-…`). **No actor, no note.** |
| Card assigned | Partly | It is an `ITEM_UPSERTED` push. The card's `assignee` changed, but the event doesn't say so or who did it. |
| Bot started or stopped working | Yes, with no card | `Push::BotState{bot_id, state, reason, at}` (`events.rs:11-16`). `working` on "prompt submitted", `ready` on "turn complete" (`supervisor/hooks.rs:32, 47`), and the desktop applies it (`useDaemonPushes.ts:55-57`). `ActivityUpdate` carries the finished turn's text (`activity.rs`). |
| Move refused | **No** | Returned to the caller only (`moves.rs:102-113`). No push, no row. |
| Draft updated | Yes | `BoardEvent{DRAFT_CHANGED}` plus the `DRAFT_UPDATED` Needs-you row. No actor or field. |
| Bot commented | Partly | `ITEM_UPSERTED` push (`mcp/board_edit.rs:67-…`). No "comment" marker. |
| Toast UI | Yes, at the bottom right | `useToasts.ts:5-6` (7 s TTL, at most 5), `Toasts.tsx`, `styles/toasts.css:3-12` (`position:fixed; right/bottom:16px`). Fed by `notify` and `approval_pending` pushes (`useDaemonPushes.ts:95-133`). Only the owner's *own* move toasts "P2-001 → Ready" (`useBoardMoves.ts:135-136`). |
| Banner strip on the board | Yes | `LeadNotice.tsx` (the lead's structure changes, View and ✕, dismissals remembered). |
| Phone | Partly | Board events reach `KanbanStore.listen` (`gravitiOS: TheHermes/Core/KanbanStore.swift:62-85`). A top-banner pattern exists (`ConnectionBanner`, `UI/ActivityViews.swift:191`), but there is no toast system. |

### Wire changes (all additive)

1. Add to `BoardEvent` (`requests.proto:737`):
   - `optional string actor = 7;` as `user`, `bot:<id>`, `device:<id>` or `daemon`;
   - `optional string note = 8;` with the move note, the fact name or the reason;
   - `optional BoardActivity activity = 9;` with a small enum
     `{MOVED, ASSIGNED, UNASSIGNED, COMMENTED, EDITED, MOVE_REFUSED, FACT}` plus
     `repeated Unmet unmet = …` for refusals and `optional string to_column` for a refused target.

   `Change`/`BoardChange` (`feed.rs:35-46, 114-…`) gain the same fields, and every
   `feed.publish` call site passes the actor (they all have it).
2. New kind `BOARD_EVENT_KIND_MOVE_REFUSED = 8`. It is published by `item_move`
   when `Moved::Refused` comes back for a **bot** actor; the owner already sees
   their own refusal. It carries `item_id`, actor, target and unmet, and no card.
   Old clients ignore unknown kinds. Check `boardSync.ts:95-111` falls through
   harmlessly, and that the phone does too. Don't bump `seq` for it, or use a
   separate stream, so old clients don't refetch on a "gap".
3. Add `optional string item_id` to the `bot_state` JSON push: the card the turn is
   about, from the last card-scoped delivery (fix D). This is additive, and old
   clients ignore it.
4. Linked computers: mirror relays carry the new fields untouched
   (`kanban/mirror.rs`). Check that the peer relay copies unknown fields; prost drops them
   unless the relay re-encodes from the struct.

### Desktop UI

- **`useBoardActivity` hook.** It subscribes to `board_event` (all watched projects)
  and to `bot_state`, and turns them into banner entries, for example:
  - "New Bot picked up P2-001 “Test”" (assigned to a bot, or bot_state working with
    item_id);
  - "New Bot moved P2-001 To do → Doing";
  - "New Bot finished a turn on P2-001";
  - "New Bot couldn't move P2-001 to Doing: Only the owner can… ⓘ";
  - "New Bot updated draft P2-002 (description)".

  Skip events whose actor is the owner on this connection; the owner already gets
  "P2-001 → Ready".
- **Placement "from the top".** Either give `Toasts` a `placement="top"` variant with a
  top-center stack under the title bar (`toasts.css`), or add a `BoardActivityBanner`
  strip next to `LeadNotice` at the top of `BoardView`, with the app-level toast as
  the fallback when the board isn't open. I recommend both: the strip on the board, and
  top toasts elsewhere for the important kinds (refused, assigned).
- **Click-through.** The action "Open" calls the existing `useOpenedItem`, the
  drawer-opening path (`components/board/useOpenedItem.ts`), with
  `initialTab:"activity"` (`DrawerTabsView.tsx:24`). From outside the board it
  switches to the project's Board tab first.
- **Card side effects.** A "working" dot on the assignee avatar while `bot_state = working`
  and `item_id` matches. Entities already hold the bot's state, so `BoardCard.tsx:172-185`
  just reads it. A transient highlight goes on the card that just moved.

### Rate limiting and grouping

- Coalesce per (actor, item) within 10 s. A move inside a turn plus turn end gives "New Bot
  moved P2-001 to Done and finished".
- Coalesce per actor across items within 10 s: "New Bot moved 4 cards".
- Show at most 3 banners at once, newest on top, transient for 6 s. Refusals and
  assignments stay until dismissed when the window is unfocused.
- Turn start and stop are shown only for turns linked to a card, never for chat
  turns, or the feature turns into noise. Each turn gets one banner at start, which is
  replaced at end.
- A per-kind preference in Settings → Notifications (`components/settings/NotificationSettings.tsx`),
  plus native notifications for "refused" and "assigned" when the app is in the
  background (reuse `notifyNatively`, `notify.ts`).

### Phone (gravitiOS)

- Decode the new `BoardEvent` fields (regenerate `hermes_board_v1_requests.pb.swift`).
- Add a `BoardActivityBanner` via `.safeAreaInset(edge:.top)` on the board and item
  screens, styled like `ConnectionBanner`, with the same grouping rules. Tapping it
  opens `ItemView`.
- Optional: an APNs push for refused and assigned while the app is backgrounded.
  This is a separate decision.

### Files to touch

- Daemon:
  - `proto/hermes/board/v1/requests.proto`
  - `crates/bus` (the generated contract)
  - `kanban/feed.rs`
  - `ws/board/item_edits.rs`, `ws/board/drafts.rs`
  - `mcp/board.rs` (`published`, `item_move` refusal publish), `mcp/board_edit.rs`
  - `kanban/api.rs` (fact moves: actor `daemon`, note = fact)
  - `kanban/drafts.rs`
  - the WS forwarder in `ws/board.rs`
  - `kanban/mirror.rs`
  - `events.rs` and `supervisor/hooks.rs` (bot_state `item_id`), with a delivery-to-item map in `delivery.rs`/`messaging`
- Desktop:
  - `protocol/gen` (regenerated), `protocol/board.ts`
  - `components/board/boardSync.ts`
  - new `components/board/useBoardActivity.ts` and `BoardActivityBanner.tsx`
  - `components/Toasts.tsx`, `styles/toasts.css`
  - `app/useDaemonPushes.ts`
  - `components/board/BoardCard.tsx`
  - stories and visual baselines
- Phone: the generated pb, `Core/KanbanStore.swift`, `Core/KanbanModels.swift`, a new
  banner view, and `UI/ItemView.swift`.

### Effort

| Part | Estimate |
|---|---|
| Daemon wire + publishers + refusal push + bot_state `item_id` | 1.5–2 days, with tests |
| Desktop hook, banner, grouping, click-through, working dot, stories, baselines | 2–3 days |
| Phone | 1–1.5 days |
| **Total** | **≈ 5–6.5 dev-days** |

Fixes A–F come on top: A plus B plus C plus the prompt part of D plus E is about 2–3
days; fix F plus the fact half of D is about 2 more.

## 5. Open questions for the owner

1. **First board preset (fix A):** Simple (To do / Doing / Done), Simple with a Review
   column, or ask at project creation? Should an existing project with no board
   get the same default?
2. **First bot = lead (fix B):** should the project's first bot become its lead
   automatically, or should assignees just get attach/update on their own cards?
3. **Automatic moves (fix D):** may the daemon move a card to Doing on its own when the
   assignee starts a turn on it, or should moves stay with the bot (prompt guidance
   only)? And who moves it to Done on a simple board: the bot, or the owner after
   reviewing?
4. **Release-gated escape hatch:** should the owner's "unlisted move with a reason" still
   skip the DoR and Start conditions (Inbox→Doing)? Or should it be shown as a distinct
   "override" in the Move menu rather than ✓?
5. **Notices as work:** should a `[card …]` notice from the owner ever count as a request to
   work, or only an explicit assignment (or task)? This decides how the prompt contradiction
   is resolved.
6. **Notifications:** which kinds should be on by default, and should turn start and stop
   be included? Banner strip on the board, top toasts app-wide, or both? Phone push
   notifications for these?
7. **Refusal history:** keep refusals only as live banners and card status, or also in
   the card's history (needs a migration)?
8. **Stale Needs-you rows:** the greeting question (`fb82d859`) and the draft question
   (`1b096d27`) are still open. Should a question about a draft close when its discussion
   ends?

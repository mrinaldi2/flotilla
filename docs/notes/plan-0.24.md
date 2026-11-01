# Flotilla 0.24: proposal (the board runs the SDLC, Commands, approvals, usage)

Draft for your review on 2026-10-20; decisions answered by the owner on 2026-10-21 (see the end). Nothing is built yet. Everything ships on
desktop and phone. The decisions you need to make are at the end, each with
my recommendation.

## Part 1: the board runs the SDLC
**Today:**
- A column's rules only check a move: who may move the card, and what it
  needs first.
- Automations only move cards on facts, such as "PR merged → Done".
- Nothing assigns anyone when a card enters a column, so the lead assigns
  every step by hand.

**Proposal:** each column gets an **On entry: assign role** setting.
- When a card enters the column, Flotilla gives it to a holder of that role,
  using the same choice as 0.23's assign-by-role: awake and free first, then
  asleep (which wakes it).
- The holder is told, exactly as if the lead had assigned the card.
- It works however the card got there: moved by a bot, by you, or by a fact
  such as "PR opened → Review".

**Default chain** for the Simple + Review and Software presets, new boards
and existing ones:

| Column | Assigns | Who |
|---|---|---|
| To do / Inbox | Product | shapes the card: why, done-when, acceptance criteria |
| Research | Architect | code research and design notes |
| Doing | Developer | builds it |
| Review | Tester | verifies it against the acceptance criteria |
| Verify (release-gated) | Tester | verifies on the computers |

Each role finishes its step by moving the card to the next column, which
assigns the next role.
- **A failed check sends the card back:** Review → Doing re-assigns the
  Developer, and the Tester's note goes with it.
- **Done:** the Tester's pass moves the card to Done. Today Done is
  lead-only and needs someone who didn't work on the card; the Tester
  qualifies, since it didn't build the card.
- **Unheld roles:** if a column's role has no holder, the card stays with
  whoever moved it. A Needs-you row says "Nobody holds Architect", with
  **Create one**.

**The lead's instructions** are rewritten: plan the board, set the roles,
spawn role workers for parallel cards, and stop hand-assigning steps.

**End to end:** a PhD card goes Product → Architect → Developer → Tester →
Done on your Mac, with at most 6 bots awake, and we watch it.

## Part 2: a Commands page in place of Main chat
**Remove Main chat:**
- **Desktop:** the ⌘J panel and the "✉ Chat" rail item.
- **Phone:** the Chat tab, and the iPad's Chat inspector.

**What stays:**
- each bot's own Chat tab;
- Reports;
- drafts' Discuss;
- the daemon's owner threads, which the dashboard, Team and Reports still
  use.

"Reply" from a report opens the bot's Chat tab instead of Main chat.

**Add a Commands page:**
- **Desktop:** a "⌘ Commands" rail item, where Chat was.
- **Phone:** a Commands tab, where Chat was.

It lists every command every bot ran, across all projects and computers.
- **Running first,** then newest.
- **Each row shows:** the bot and project, the command and its description,
  whether it's background, its status (running, done, failed, stopped), when
  it started, how long it took, and its exit code.
- **Opening a row** shows the last output, and the card or task it was for.
- **Filters:** project, bot, status (running, failed, all) and text search.
- **A summary on top:** "7 running · 3 failed today", per computer.
- **Updates live:** a push when a bot's commands change, instead of each app
  polling every 3–10 s.
- **Stop:** a **Stop** button on a running background command, owner only
  (decision 4).

**Daemon:** `list_commands {project_id?, bot_id?, status?, query?, limit}`
- gathers from every bot, including linked computers' bots;
- comes with a `commands_changed` push;
- `list_bot_commands` stays for older apps.

A design of the page for desktop and phone comes with this proposal.
**Design approved by the owner on 2026-10-20:** https://claude.ai/artifact/1dkyHsKgdHChUuiP1rTQZD (desktop page, phone list, phone detail).

## Part 3: approvals without the chat
**A Push button.** A bot that needs a push it may not do itself asks for
one: `request_push {branch, target?, item?, reason}`.
- **The card:** you get **Push** or **Reject**:
  - on the card;
  - in Needs you;
  - on the plan gate when its document isn't pushed yet.
- **On Push:** Flotilla runs that exact `git push` as you, audited. It
  reuses 0.17's owner actions: single use, hashed, and it expires in 24 h.
- **What stays the same:** bots still can't push to `main`. A push request
  to `main` is a release, not a button.
- **Phone:** it can run owner actions too. Today it only counts them.

**Approve / Send back at every plan step.**
- **Research:** it becomes a gate. When every spike is done, research waits
  for your **Approve findings / Send back**, where today it closes by itself.
- **All the same:** Research, PRD, design, HLD and LLD get one panel with
  the document, the questions answered, the children it creates, and Approve
  or Send back with a note. The panel is the same on desktop and phone.

## Part 4: usage and cost, with a limit per project
**Today:** nothing measures usage. Every bot's Claude Code session records
it per message (input, output and cache tokens, and the model), but no part
of Flotilla reads it.

**Proposal:**
- **Count it:** Flotilla reads each bot's transcript as its turns end, plus
  Codex's usage events, into a table per bot and day (tokens by kind, model
  and turns). That includes role workers and routines.
- **Show it:**
  - a **Usage** page, desktop and phone: per project and per bot, for today,
    7 days and 30 days, with a share of the total;
  - a small "usage today" line on each project card;
  - an estimated cost from a price table you can edit, marked as an
    estimate.
- **A limit per project:** a weekly token budget, as a number or as a share
  of all Flotilla usage (decision 6).
  - **At 80%:** a Needs-you row warns you.
  - **At 100%:** the project's on-demand bots stop waking ("Budget reached"
    in the queue). The lead and always-on bots keep running unless you choose
    otherwise.
  - **Raise or extend:** Needs you offers **Raise budget** and **Extend
    today**.
- **Rate limits become visible:** a bot that hits Claude's rate limit shows
  as Rate limited. That state exists today, but nothing sets it.

## Proposed steps
1. **Contract:**
   - `on_entry` role per column;
   - `list_commands`, `commands_changed` and `stop_command`;
   - `request_push`, the push owner action, and owner actions on the phone;
   - research as a gate;
   - `usage_get`, `set_project_budget`, the budget attention rows and the
     `rate_limited` state;
   - capabilities `board_flow_v1`, `commands_v1`, `push_requests_v1` and
     `usage_v1`.
2. **Daemon, flow:** on-entry assignment, the presets' default chain, the
   Done rule for the Tester, unheld-role rows, and the lead's prompt.
3. **Daemon, commands and approvals:** the cross-bot command list and its
   push, Stop, `request_push`, and the research gate.
4. **Daemon, usage:** transcript and Codex ingestion, the usage table, the
   budget and its enforcement in the wake queue, and rate limiting.
5. **Desktop:**
   - remove Main chat;
   - the Commands page;
   - the column On-entry setting;
   - the gate panel at every step;
   - Push cards;
   - the Usage page and budgets;
   - Ladle stories.
6. **Phone:** the Commands tab in place of Chat, Push and owner actions,
   gate panels, the Usage page, and the version bump.
7. **End to end on your Mac and iPhone:**
   - a PhD card flows through the four roles;
   - a push approved from the phone;
   - the research gate;
   - a project hitting its budget;
   - then the release.

Steps 2, 3 and 4 can run in parallel once step 1 is merged, and so can 5
and 6.

## For you to decide
1. **Default chain:** To do → Product, Research → Architect, Doing →
   Developer, Review → Tester, Verify → Tester (recommended). Or Review →
   Architect for code review, with Verify → Tester only on release-gated
   boards.
2. **Done after a pass:** the Tester moves a passed card to Done
   (recommended), or the lead still closes every card.
3. **"All bot chat":** I read it as Main chat (the ⌘J panel and the phone's
   Chat tab), which the Commands page replaces (recommended). Bot-to-bot
   Conversations (a project tab) stays. Should Conversations go too?
4. **Stop on the Commands page:** Stop kills a running background command's
   process (recommended, owner only), or the page is read-only in 0.24.
5. **Research gate:** always on (recommended, it is what you asked), or a
   per-project setting that defaults to on.
6. **Budget unit:** weekly tokens per project, with an estimated cost shown
   (recommended), or a share of the total (for example PhD 60%, Books 20%),
   or an estimated dollar amount.
7. **At 100% of the budget:** on-demand bots stop waking and the lead keeps
   running (recommended); or the whole project pauses, the lead included; or
   it only warns.

## Owner's answers (2026-10-21)
1. **Default chain:** the recommended one: To do → Product, Research → Architect, Doing → Developer, Review → Tester, Verify → Tester.
2. **Done after a pass:** the Tester moves a passed card to Done.
3. **"All bot chat":** Main chat, meaning the ⌘J panel and the phone's Chat tab, which Commands replaces. Conversations stays.
4. **Stop:** the owner's Stop kills a running background command's process, and **the bot is told**. It gets a message saying the owner stopped its command, naming the command and the card or task it was for, so it doesn't retry blindly.
5. **Research gate:** on by default, with a per-project setting to turn it off. This is a change from "always on".
6. **Budget unit:** weekly tokens per project, with an estimated cost shown.
7. **At 100%:** on-demand bots stop waking and the lead keeps running.

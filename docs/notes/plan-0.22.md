# Flotilla 0.22: plan (questions and artifacts)

Approved by the owner on 2026-10-18 "as recommended" (choices below). Work
starts once 0.21 ships.

- **Branch:** `release-0.22.0`, cut from main once 0.21.0 is tagged.
- **Steps:** one PR into it per step, each with the PR watch on.
- **Merge:** you review the release PR before it goes to main, as with 0.21.
- **Phone:** one PR on `flotilla-ios` for the 0.22 contract.

## What was decided
- **Questions:**
  - Each question is a decision-registry entry of kind `question`, so there
    is one list for questions and decisions.
  - Answers are written back into the research document automatically.
  - A blocking question holds both research closing and gate submission.
- **Artifacts:** each kind of file has one home.
  - **The card:** work made for a card is attached to it.
  - **The repository:** specs and code.
  - **Artifacts:** shared project files: shared files, uploads and checks,
    listed with the cards that use them.
  - **The bot's workspace:** scratch.
- **Not deleted:** no existing file, question or comment.

## Steps, in order

**1. Contract** (`proto/`, `docs/protocol.md`, the generated TS, and the
vendored contract on the phone)
- **Decisions:** `question` entries carry their anchor (`item_id`,
  `plan_node_id`), `asked_by`, a `blocking` flag and one answer (the ruling
  option and/or text, by whom and when).
- **Board:** `ItemDetail.questions` and `PlanNodeDetail.questions`, a
  `QUESTION_CHANGED` push, and `question_answer` / `question_dismiss` on the
  board's WebSocket, answered with the `approve` grant.
- **Artifacts:**
  - `list_artifacts {group: shared | uploads | checks}`;
  - the storage folders (`items/`, `.cache/`) are never listed;
  - each file gets `used_by: [card ids]`.
- **Capabilities:** `questions_v1` and `artifacts_v2`. Older apps keep working
  without them.

**2. Daemon: questions**
- **Migration** (append-only): question rows on the decision table, and links
  to cards and plan nodes.
- **MCP:**
  - `ask_owner {item? | node?, questions: [{text, options?, recommendation?, why?, blocking?}]}`
    makes one entry per question and a card comment that links them;
  - `question_list` returns the questions.
- **Answering:**
  - the bot that asked is told each answer, and so is the lead for plan work;
  - the answer is appended under "Answers from the owner" in the spike's
    research document (the attached file, as a new revision);
  - the gate submission lists the questions answered for that node.
- **Blocking:** research can't close, and `plan_submit` is refused
  (`plan.question_open`), while one of the node's blocking questions is open.
- **Needs you:** one row per open question, answerable inline. The project
  card's count includes them.
- **Old bots:**
  - `item_comment {asks_owner}` and `message_owner {asks}` keep working.
    Existing open rows show as one question each, answered by a comment as
    before;
  - prompts tell bots to use `ask_owner`, with one question per entry.

**3. Daemon: artifacts**
- **Listing:** `list_artifacts` gains groups, `used_by`, and the hidden
  storage folders.
- **One Documents list:** a card's `item_link` to an artifact appears with its
  attachments as a document that opens in place, so a card has one Documents
  list.
- **Prompts:** work for a card goes on the card, files shared between cards
  go in artifacts, and scratch stays in the workspace.
- **Tidy-up read:** a list of loose artifacts no card uses. Nothing is moved
  or deleted.

**4. Desktop**
- **Card drawer:** a Questions section above the comments, with open ones
  first, option buttons and a reply box.
- **Plan tab:** each node's open questions under Research.
- **Project:** a Questions list, open and answered, filterable by card or
  node.
- **Needs you:** question rows you can answer inline.
- **Artifacts:** grouped into Shared, Uploads and Checks, with chips for the
  cards that use each file. The tidy-up list offers keep, attach to a card,
  or remove (with a confirm).
- **Card documents:** links and attachments shown as one Documents list.
- **Checks:** Ladle stories and visual baselines; the fallow and oxlint checks
  stay green.

**5. Phone** (`flotilla-ios`, after step 1's contract is vendored)
- **Card:** a Questions section where you answer inline.
- **Project and Needs you:** the project's ⋯ menu gets Questions, and Needs
  you rows can be answered.
- **Artifacts:** the same groups as desktop, plus the card's Documents list.
- **Tests:** UI tests against the demo (it seeds questions and grouped
  artifacts), plus the version bump.

**6. End-to-end on your install, then the release**
- **On your Mac and iPhone:** a bot asks three questions on a research spike.
  You answer one on the phone and one on the desktop. The research document
  gets the answers, and the gate waits on the blocking one. Artifacts show
  grouped.
- **Docs:** `docs/planning.md` and `docs/protocol.md`, plus release notes.
- **Release:** the release PR to main for your review, then the 0.22.0 tag
  and publish, and the phone build.

## Order and parallel work
Step 1 comes first. Then the work can run in parallel:
- 2 and 3 (daemon);
- 4 (desktop) and 5 (phone), against step 1's contract and a demo daemon.

Step 6 runs once 2 to 5 are merged into `release-0.22.0`.

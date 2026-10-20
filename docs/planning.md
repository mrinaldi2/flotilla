# Planning (0.21.0)

A project can be planned: goals hold initiatives, initiatives hold epics,
epics hold features, and features hold the task cards that build them. A
feature is never created as a card. It is born when the owner approves its
epic's HLD. The approved plan is #43, and the design canvas is linked from
it.

Planning is opt-in per project (`plan_enable`). It needs the project's
repository, because the documents live there. A project without planning
keeps its boards, and cards still have the four 0.21 types.

## Levels, stages and documents

| Node | Stages (gates in bold) | Document | Made by |
|---|---|---|---|
| Goal | define → active → met | `docs/goals/<goal>/goal.md` | the owner |
| Initiative | (proposed) → research → **PRD** → running → done | `docs/initiatives/<i>/prd.md` | the owner, or the lead proposes and the owner accepts |
| Epic | research → **design** → **HLD** → running → done | `…/epics/<e>/design/design.md`, `…/hld.md` | the owner approving its initiative's PRD |
| Feature | **LLD** → build → released | `…/features/<f>/lld.md` | the owner approving its epic's HLD |
| Task, design, bug, spike | the board's columns | one PR each | `item_create` on the initiative's board |

- **Gates.** The lead writes a gate's document in the repository and submits
  it (`plan_submit`) with the children it proposes: epics with a PRD,
  features with an HLD, task cards with an LLD. The owner approves it
  (`plan_rule`), leaving out any child, or sends it back with a note. The
  approval pins the document's commit. A node whose approved file changed
  since then says so, and the lead resubmits it.
- **Research** is a tracked stage. An initiative or epic starts in research,
  and its gate can't be submitted until every spike of it (cards with
  `plan_node_id` set to the node) is done with a research document attached
  (`item_attach`), which the owner reads on the card. Only the owner skips
  research, with a reason. The lead may ask (`plan_research_skip`).
- **Questions (0.22.0).** A bot asks the owner with `ask_owner`, one
  record per question, on a card and/or a node. The owner answers each once
  on the card, the plan node, the project's Questions list or in Needs you,
  or dismisses it with a reason. The bot that asked is told, and so is the
  lead for plan work. An answer on a research spike is added under "Answers
  from the owner" in its research document, as a new revision of an
  attached file. A `blocking` question keeps the node's research from
  closing and refuses `plan_submit` (`plan.question_open`) until it is
  answered or dismissed.
- **Kickoffs.** Whatever a node waits on the lead for comes to the lead as
  a card assigned to it, labelled `kickoff`: "Propose initiatives for" an
  active goal (on the project's default board), "Research" for an
  initiative or epic in research (a spike, which counts toward closing
  research), "Write the PRD/design/HLD/LLD" for a document stage, and
  "Revise" after a send-back, carrying the owner's note. A document's card
  closes once the node moves on, research's once research is skipped, and
  the proposals card once the goal is met or dropped. Nothing is made twice,
  and kickoffs don't count toward a node's progress.
- **Boards.** Each initiative gets its own board, and the cards of its
  subtree go there with `plan_node_id`.
- **Closing.** A feature in build is released once every card of it is done
  (a cancelled card aside). On a release-gated board, Done means shipped. An
  epic is done once its features are released, and an initiative once its
  epics are done. A goal is the owner's to mark met.
- **Turning planning on or off** rewrites every bot's prompt and tells each
  bot once.
- **Guidelines** in `docs/guidelines/` are listed with the plan, and every
  bot is told to read them before it works.

## Cards in every project

From 0.21.0, cards are **task**, **design**, **bug** and **spike**. Epics and
features moved to the plan, and chores became tasks (`new.type_retired`
refuses the old types). Every board has a **Research** column between its
open columns and Doing. A task or bug enters Doing once its "Code research"
section is filled or a research document is attached (`start.research`). A
done design ticket is linked to the task that builds it (`design`).

## Upgrading to 0.21.0

The upgrade **deletes every epic and feature card**, with its comments,
history, links, attachments and release rows. This was the owner's decision
in #43, and the plan replaces those cards. Their child cards stay and lose
their parent. Chores become tasks.

Before restarting the daemon on 0.21.0, list what it will remove:

```bash
flotillad board retired-cards
```

The daemon backs up the database before migrating (`backups/pre-migration-*`).
After the upgrade it deletes the removed cards' attachment folders and
tells each one's assignee, once. Every existing board gains the Research
column the first time the daemon starts. A board whose owner removes it
afterwards keeps it removed.

Apps and phones from before 0.21 keep working against a 0.21 daemon,
except that creating a feature, epic or chore is refused with a clear
message. They don't show the plan.

## Where it lives

- Contract: `proto/flotilla/board/v1/plan.proto`, and the plan section of
  [protocol.md](protocol.md#the-plan-0210).
- Daemon: `crates/flotillad/src/plan/` (the `Planner` service),
  `db/plan*.rs`, `ws/board/plan.rs`, `mcp/plan.rs`, `attention/plan.rs`.
  Migrations 072 (card types) and 073 (the plan).
- Bots' prompts: `paths/prompt_plan.rs`.

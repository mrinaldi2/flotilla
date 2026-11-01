# Questions for the owner: proposal for 0.22 (2026-10-18, not built)

## What happened on ASL-004
The lead finished the sources & licensing research and wrote one comment on the
card, marked as asking the owner (`item_comment {asks_owner}`). That one comment
held the findings and three questions:
1. Should v1 cover only the ~200–250 celebrated saints, or the full Martyrology?
2. Italy plus the universal calendar only?
3. May we contact the LEV and the CEI about licensing?

The daemon recorded **one** question for the whole comment. That gave one
Needs you row, closed by any comment you write on the card. A second comment
then said "I've asked you 3 questions on the card". So:
- you can't answer one question and leave the others open;
- answering means writing a comment that has to cover all three;
- nothing records which answer belongs to which question;
- the answers never reach the research findings or the plan.

## Three ways to ask today
| Way | Where it lands | Ends when | Problem |
|---|---|---|---|
| `item_comment {asks_owner}` | a card comment + 1 Needs you row | you comment anything on the card, the card closes, or you dismiss it | one row per comment, not per question; no answer field |
| `message_owner {asks, item?}` | the bot's thread (+ a card comment) + 1 Needs you row | you write anything in that thread | lives in chat; nothing ties it to the card's work |
| `raise_decision {kind: question\|decision, item?}` | the Decisions registry, linked to the card | you answer it | right shape (one ruling, options, a recommendation, tags, history), but bots use it for big rulings only, and the card only shows a link |

## Proposal: one question, one object
**A question is its own record.** Bots ask with the new tool
`ask_owner {item? | node?, questions: [...]}`. Each entry has:
- `text`
- optional `options`, plus the bot's `recommendation`
- optional `why` (one line)

The call creates **one record per question**. It is a decision-registry entry
of kind `question`, so it keeps the registry's history, tags, authority and
phone approval. Each record has:
- who asked, when, and the card and/or plan node it belongs to;
- a state: `open`, then `answered`, or `dismissed` with a reason;
- **one answer**: the option you picked and/or your text, with who and when.

The card's comment links to its questions and no longer carries them.

**One place to ask.**
- `item_comment {asks_owner}` and `message_owner {asks}` stay for older bots.
  The prompt tells bots to use `ask_owner`, one question per entry.
- `raise_decision` stays for rulings (options with grants, deadlines).
- A question asked through `ask_owner` with options is the same kind of
  record, so decisions and questions share one list.

**Answering, wherever you are.**
- **Card (desktop drawer and phone card):** a *Questions* section above the
  comments lists open questions first. Each one has its option buttons and a
  reply field, and you answer it right there.
- **Plan node (Plan tab):** the open questions of its spikes and cards, under
  Research.
- **Project:** a *Questions* list (open / answered), filterable by card or
  node. On the phone it's in the project's ⋯ menu.
- **Needs you:** one row per open question, each answerable inline. The
  project card's "Needs you · N" counts open questions.

**Answers go back where they matter.**
- The bot that asked is told each answer as it comes, with the card and
  question, and so is the lead for plan work.
- On a research spike, the answers are added under *Answers from the owner*
  in its findings document. A plan node's gate submission lists the answered
  questions with it, so the PRD/HLD records what you decided.
- Research can't close, and a gate can't be submitted, while a question its
  document depends on is open. The bot marks such a question `blocking`;
  others don't hold anything up.

**Contract and apps (0.22).**
- **Proto:** `Question` in `flotilla.board.v1`, carried on the card detail
  and the plan node detail.
- **Requests and pushes:** `question_answer`, `question_dismiss` and a
  `QUESTION_CHANGED` push.
- **MCP:** `ask_owner` and `question_list`.
- **Migration:** existing open `card_question` / `owner_question` rows show as
  one question each, with the comment as the text. They are answered by a
  comment as before, and nothing is deleted.
- **Desktop and phone:** the card section, the project list and the Needs you
  rows.

## For you to decide
1. **Shape:** should questions be decision-registry entries of kind
   `question` (recommended: one list, already on the phone, with authority
   and history), or a separate card-only table?
2. **Who sees answers:** should answers be written back into the research
   document automatically, or only offered to the lead to add?
3. **Blocking:** should a blocking question hold research and gates, as
   above, or only show a warning?

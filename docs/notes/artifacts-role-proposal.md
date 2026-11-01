# What artifacts should be for (proposal for 0.22, 2026-10-18; final, not built)

## Today
A project's `artifacts/` folder holds four unrelated things, and the
Artifacts lists in the desktop and phone show them all flat, newest first:

- **Bot output.** Notes, reports, frames and tables a bot wrote. PhD has
  196 loose files plus run folders, 38 MB.
- **Card attachments' storage** (`items/<card>/`), which the card already
  shows.
- **Owner uploads** (`uploads/`).
- **Daemon output:** check logs (`checks/`) and a linked computer's
  attachment cache (`.cache/`).

A card can also point at an artifact in two ways that look different:
- an `item_attach` doc, which shows under Attachments and opens in place;
- an `item_link` of kind artifact, which shows only under the drawer's
  Links tab.

The research for ASL-004 was the second kind, so it read as "dumped in the
artifacts folder".

## Proposal: one home per kind of file
1. **The card holds its work.** A file made for a card (research,
   screenshots, reports, a design spec) is attached to that card. The
   owner reads it there, and so does the Plan tab: a spike's research
   shows under its node's Research.
   - Bots are told "a file for a card is attached to it". This part is
     done for research in 0.21.
   - An `item_link` to an artifact shows with the attachments, as a
     document that opens in place, so there is one Documents list on a
     card instead of two.
2. **The repository holds what lasts.** That means specs and decisions
   (PRD, design, HLD, LLD, guidelines) and code, with history and review,
   read through the Plan tab. Research can be copied there when it should
   outlive the card.
3. **Artifacts hold project files no single card owns.** That covers
   shared references, datasets, builds, check logs and the owner's uploads.
   - The Artifacts list groups these as Shared, Uploads and Checks.
   - It hides the storage folders (`items/`, `.cache/`).
   - Each file shows who made it and which cards use it.
4. **The bot's workspace holds its scratch.** Drafts and intermediate files
   stay in the bot's own folder and are never published to artifacts.

## What to change (0.22, after your OK)
- **Daemon:** a `list_artifacts` filter (`shared`/`uploads`/`checks`)
  that never lists `items/` or `.cache/`, plus "used by cards" (the cards
  that link or attach the file).
- **Desktop and phone:**
  - the Artifacts list grouped as above, with "on cards" chips;
  - a card's artifact links shown as documents next to its attachments.
- **Prompts:** deliverables go on the card, files shared across cards go in
  artifacts, and scratch stays in the workspace.
- **Tidy-up, nothing deleted:** a list of loose artifacts no card uses, for
  you to keep, attach to a card or remove.

Questions a bot asks you about a card are a separate proposal, also for 0.22:
`questions-proposal.md` in this folder.

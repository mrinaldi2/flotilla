# Flotilla 0.23: plan (team on demand, lighter checkouts)

Approved by the owner on 2026-10-19. The decisions are recorded at the end;
where they differ from the draft, the text above follows them.

## Why
**Memory.** The PhD team has 13 bots running all the time. Each one is a
Claude Code process of about 280 MB plus a browser helper (Playwright) of
about 85 MB. That is roughly 4.8 GB of your 24 GB, whether or not anyone is
working.

**Disk.** Bots make their own git worktrees, and each one carries its own
build output (Rust `target/`, `node_modules`). Flotilla's own development
checkouts on this Mac add up to more than 25 GB.

**What the code does today:**
- Every bot is always on, and nothing ever stops one except archiving it.
- No limit applies to how many bots run at once. Only the number of bots per
  project is limited.
- A message to a stopped bot waits in the queue. Nothing wakes the bot.
- A routine only works while its bot is running.
- The lead assigns cards by hand. There are no Product or Architect roles yet.
- Temporary workers already come and go: they are spawned, take one task,
  then retire and are cleaned up. That machinery is the base for this
  release.

## Part 1: team on demand
### How a project's team works
- **Lead:** always on. It plans, splits the work into cards and assigns each
  card to a role or a member.
- **Always-on members:** the ones you choose to keep running, for example a
  bot with frequent routines or one you chat with all day.
- **On-demand members, the new default:** Product, Architect, Developer,
  Tester, and project roles such as Thesis writer or Unity dev.
  - They keep their name, memory (`FACTS.md`), instructions and conversation.
  - They **sleep** when they have nothing to do, and **wake** when work
    arrives.
  - Waking resumes the same conversation (`--continue`), so to the bot a
    sleep is just a gap.
- **Temporary workers:** as today, they are for extra parallel work. For
  example, the lead spawns two more developers from the Developer member's
  profile for a burst of cards.
  - New: a worker inherits its role's instructions and model.

### Sleeping and waking
- **A new bot state, Asleep:** no processes at all, and it isn't an error.
  The apps show it as ◌ Asleep with a "Wake now" button.
- **Wakes on:**
  - a card assigned to it, or to its role;
  - a task or DM to it, including from the lead;
  - a review request;
  - a routine falling due;
  - you opening its chat.
- **Sleeps after** 10 idle minutes: no open task, no card of its in Doing,
  nothing queued for it, and no turn running.
- **Running cap:** at most 6 bots run at once on this Mac (Settings). When the
  cap is full, a wake waits in a queue.
  - **Queue order:** work keeps the board's priority order (priority, then
    age) at the moment it queues. A ticket that arrives later waits behind
    work already queued, even when its priority is higher.
  - **The one exception:** an **urgent bug** goes straight to the front.
  - **Who goes first:** you opening a chat, then the lead, then board work.
  - The apps show "4 of 6 running · 2 waiting".
- **Fairness for sleeping bots:**
  - The keeper doesn't nag a bot that is asleep or waiting for a slot.
  - A task's deadline doesn't count time spent queued for a slot.

### The lead's standard way of working
- **The lead's instructions are rewritten:**
  - The team is a roster of roles.
  - Assign each card to the role that owns the next step: Product shapes it,
    Architect designs it, Developer builds it, Tester verifies it.
  - Flotilla wakes whoever holds that role.
  - Spawn workers only to add parallel hands.
  - Never keep a member busy just to keep it awake.
- **Assigning to a role:**
  - It picks a holder who is awake and free, otherwise one who is asleep (and
    wakes it).
  - When every holder is busy and the lead allows it, it spawns a worker from
    that role.
- **Roles:** Product and Architect are added to the board roles; Dev, Tester
  and the reviewers already exist. The lead may set them as it can set Dev
  today.

### Saving memory beyond sleeping
- **Browser helper:** started only for bots whose role uses a browser (Tester,
  UX), or when one asks for it, instead of for every bot.
  - That saves about 85 MB per bot even while bots are awake.

### Moving your current teams
- **On upgrade:** the lead stays always on, and every other bot becomes on
  demand. Nothing is deleted, and no bots are merged.
- **Parallel work:** the lead creates similar bots (workers from the same
  role) when two tasks can be worked on in parallel.

## Part 2: lighter checkouts (ideas from Anthropic's Scion)
The article describes Scion, a server-side version control system for one
enormous monorepo, and says itself that it is no replacement for git in
general use. What applies to one Mac is its approach:
- one shared store of history, instead of a full copy per checkout;
- working copies are optional, partial, and made only for those who write;
- caches are shared, bounded and content-addressed.

Applied to Flotilla:
- **One mirror per project repository**, `~/.flotilla/repos/<project>.git`,
  kept fetched by the daemon. All of the project's worktrees share its
  objects.
- **The daemon manages worktrees** (the code exists but is unused today):
  - It creates a worktree when a bot starts a code card.
  - It removes the worktree once the card's PR merges, or salvages it when the
    card is cancelled.
  - Bots no longer make or clean their own.
  - One worktree per card, not per bot.
- **No worktree for roles that only read:** Product, Architect, Tester and
  the reviewers read the repository through the daemon instead:
  - new read-only tools `repo_file`, `repo_diff`, `repo_log` and `repo_grep`
    run against the mirror at any branch or commit;
  - a Tester that must run the code gets a worktree like a Developer.
- **Partial checkouts:** a card can name the folders it touches, and its
  worktree is a sparse checkout of just those.
- **Shared build caches:**
  - Rust: one `sccache` per Mac, instead of a 1–3 GB `cargo-target` per bot.
  - Node: one pnpm content store, hard-linked into each worktree.
  - New worktrees start from a warm seed copied with APFS `clonefile`, so
    unchanged build output costs no extra disk.
- **A disk budget per project.** The hourly disk report already exists, and
  over budget the daemon cleans worktrees of merged cards first.
- **Not in scope:**
  - a new version control system;
  - a virtual file system (FUSE);
  - replacing GitHub. Git stays.

## Proposed steps
1. **Contract:**
   - the bot's `presence` (always_on or on_demand);
   - the `ASLEEP` state;
   - `bot_wake`, `bot_sleep` and the team's running count;
   - the `max_running` and `idle_minutes` settings;
   - assigning a card to a role;
   - the read-only repo tools;
   - capabilities `team_on_demand_v1` and `repo_mirror_v1`, so older apps keep
     working and see Asleep as Stopped.
2. **Daemon, on demand:**
   - saved presence, and start only always-on or wanted bots;
   - sleep when idle, wake when delivery defers, and wake on routines;
   - the running cap and its queue;
   - the keeper and deadline fixes;
   - the upgrade migration.
3. **Daemon, team:**
   - Product and Architect roles, assigning by role, workers inheriting their
     role;
   - the lead's prompt rewrite;
   - the browser helper only where a role needs it.
4. **Daemon, checkouts:**
   - the project mirror, worktrees per card, the repo read tools;
   - sccache, the pnpm store and clonefile seeds;
   - the disk budget.
5. **Desktop:**
   - Asleep and Wake now in the bot list and chat;
   - an Always on / On demand switch;
   - the running counter and its queue;
   - the Settings for the cap and idle time;
   - Ladle stories.
6. **Phone:** Asleep and Wake, the running counter, presence in the bot
   sheet, and the version bump.
7. **End to end on your Mac:**
   - the PhD team runs on demand for a day;
   - a card flows from Product to Architect to Developer to Tester, with at
     most N bots awake;
   - before and after numbers for memory and disk;
   - then the release.

Steps 2, 3 and 4 can run in parallel once step 1 is merged, and so can 5 and
6. If you'd rather keep 0.23 smaller, part 2 (steps 4 and its UI) can move
to 0.24.

## Decided (2026-10-19)
1. **Upgrade:** every bot except the lead becomes on demand.
2. **Running cap:** 6 for now.
   - Ticket priority is respected: queued work keeps the board's priority
     order.
   - A newer, higher-priority ticket jumps the queue only when it is an
     urgent bug.
3. **Idle time:** 10 minutes for now.
4. **SDLC roles:** one on-demand member per role, with workers only for extra
   parallel hands.
5. **Routines:** a routine falling due wakes its on-demand bot.
6. **Scope:** both parts in 0.23.
7. **No merges of existing bots:** the lead creates similar bots when two
   tasks can run in parallel.

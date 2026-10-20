# The team on demand (0.23.0)

From 0.23 a project's bots don't all run at once. The lead is always on; the
rest of the team sleeps when it has nothing to do and wakes when work
arrives. A computer runs at most a few bots at a time, and work for the
others waits in a queue. The wire contract is in
[protocol.md](protocol.md#the-team-on-demand-and-lighter-checkouts-0230)
(capability `team_on_demand_v1`).

## Presence

Every bot is **always on** or **on demand**.

- **Always on:** it runs all the time, as every bot did before 0.23. The
  project's lead is always on and can't be switched. Keep a bot always on
  when you chat with it all day.
- **On demand:** it sleeps when idle and wakes when work arrives. It keeps
  its name, instructions, memory (`FACTS.md`) and conversation: waking
  resumes the same session, so to the bot a sleep is a gap.

Upgrading to 0.23 makes every existing bot on demand except each project's
lead. A new bot is on demand. Switch one in the bot's settings (desktop) or
its sheet (phone): **Always on / On demand**.

## Sleeping and waking

An on-demand bot goes to sleep after **10 idle minutes** (Settings): no turn
running, no open task, no card of its in an active column, and nothing
queued for it. Asleep is not an error: the bot list shows ◌ **Asleep**, and
no "stopped" row is raised.

It wakes when:
- a card is assigned to it, or to a role it holds;
- a task, a DM or a card notice is delivered to it;
- it is asked for a review;
- one of its routines falls due;
- you open its chat or press **Wake now**.

**Sleep now** sends an on-demand bot to sleep once its running turn ends.

## The running cap and the queue

At most **6 bots** run at once on a computer (Settings, 1–32), always-on
bots included. A wake with no free slot waits as ◌ **Waiting for a slot**,
and starts when a slot frees. The queue starts, in order:

1. what **you** woke (opening a chat, Wake now);
2. the **lead**;
3. **card work**, in the board's order when it queued: priority, then the
   card's age. Work that queues later waits behind work already queued, even
   at a higher priority. The one exception is an **urgent bug** (a P0 bug),
   which goes to the front of the card work;
4. everything else (messages and routines without a card), as it came.

The team view shows "4 of 6 running · 2 waiting" and the queue. A task's
deadline doesn't count the time spent waiting for a slot, and the keeper
doesn't nag a bot that is asleep or waiting.

## Roles and the lead's way of working

The board's roles gain **Product** and **Architect** beside Dev, Tester and
the reviewers; the lead may set them. The lead works the board by role:

- it assigns each card to the role that owns its next step: Product shapes
  it, Architect designs it, Developer builds it, Tester verifies it
  (`item_assign {role}`, or **Assign to role…** on a card);
- Flotilla gives it to a holder of the role that is awake and free, else to
  one that is asleep, which wakes; a role nobody holds is refused;
- when two cards of one role can be worked at once, the lead spawns a
  **worker from that role** (`spawn_worker {role}`, see
  [workers.md](workers.md)): a temporary copy with the role member's
  instructions and model, which retires when its task closes.

## The browser helper

On a project with a board, a bot gets its own browser (Playwright) only when
it holds **Tester** or **UX reviewer**, or may drive your Chrome. Every
other bot saves the helper's memory even while awake.

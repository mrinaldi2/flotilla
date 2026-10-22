// The team on demand and lighter checkouts (0.23.0): a bot's presence,
// waking and sleeping it, this computer's running count and its queue for a
// slot, and the project's disk budget. Without the capabilities a daemon
// runs every bot all the time, and nothing here is offered.

import type { Bot, TeamStatus, WakeReason } from "./entities";

/** Presence, Asleep, wake and sleep, the running cap and assigning by role. */
export const TEAM_ON_DEMAND_V1 = "team_on_demand_v1";
/** Columns assign a role on entry (`on_entry`), and `ROLE_UNHELD` rows (0.24.0). */
export const BOARD_FLOW_V1 = "board_flow_v1";
/** The project mirror, card worktrees and the disk budget. */
export const REPO_MIRROR_V1 = "repo_mirror_v1";

export function teamOn(api: { readonly capabilities: readonly string[] }): boolean {
  return api.capabilities.includes(TEAM_ON_DEMAND_V1);
}

export function mirrorOn(api: { readonly capabilities: readonly string[] }): boolean {
  return api.capabilities.includes(REPO_MIRROR_V1);
}

/** Asleep, or woken and waiting for a slot: "Wake now" applies. */
export function resting(bot: Pick<Bot, "state">): boolean {
  return bot.state === "asleep" || bot.state === "queued";
}

/** An on-demand bot that is up: "Sleep now" applies. */
export function canSleep(bot: Pick<Bot, "state" | "presence">): boolean {
  return (
    bot.presence === "on_demand" &&
    !resting(bot) &&
    bot.state !== "stopped" &&
    bot.state !== "stopping"
  );
}

/** "4 of 6 running · 2 waiting". */
export function runningText(team: TeamStatus): string {
  const waiting = team.queue.length;
  const running = `${team.running} of ${team.max_running} running`;
  return waiting === 0 ? running : `${running} · ${waiting} waiting`;
}

/** Why a bot waits, as the queue reads it. */
export const WAKE_REASON_LABEL: Readonly<Record<WakeReason, string>> = {
  owner: "You woke it",
  lead: "Lead",
  card: "Card",
  task: "Task",
  message: "Message",
  review: "Review",
  routine: "Routine",
};

/** "waiting 4 min", "waiting 2 h 05 min": how long a wake has queued. */
export function waitedText(queuedAt: string, now: number): string {
  const then = Date.parse(queuedAt);
  if (!Number.isFinite(then)) {
    return "";
  }
  const minutes = Math.max(0, Math.floor((now - then) / 60_000));
  if (minutes < 1) {
    return "waiting under a minute";
  }
  if (minutes < 60) {
    return `waiting ${minutes} min`;
  }
  return `waiting ${Math.floor(minutes / 60)} h ${String(minutes % 60).padStart(2, "0")} min`;
}

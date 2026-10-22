// Wording for the Commands page (0.24.0, `commands_v1`): status labels,
// durations, start times and the per-computer summary. Pure, so tested alone.

import type { CommandsReply } from "../../protocol/commands";

/** One command of `list_commands`, with its bot, project and computer. */
export type CommandRow = CommandsReply["commands"][number];
type CommandSummary = CommandsReply["summary"][number];

/** "12s", "4m 03s", "1h 05m". */
export function elapsed(from: string, to: string): string {
  const seconds = Math.max(0, Math.round((Date.parse(to) - Date.parse(from)) / 1000));
  if (seconds < 60) {
    return `${seconds}s`;
  }
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) {
    return `${minutes}m ${String(seconds % 60).padStart(2, "0")}s`;
  }
  return `${Math.floor(minutes / 60)}h ${String(minutes % 60).padStart(2, "0")}m`;
}

/** How long it ran: to its end, or to `now` while it runs. */
export function took(row: Pick<CommandRow, "started_at" | "ended_at">, now: number): string {
  return elapsed(row.started_at, row.ended_at ?? new Date(now).toISOString());
}

interface StatusLabel {
  readonly glyph: string;
  readonly text: string;
}

/** "◑ Running", "✓ Done", "✗ Failed · exit 1", "⊘ Stopped by you". */
export function statusLabel(
  row: Pick<CommandRow, "status" | "exit_code" | "stopped_by_owner">,
): StatusLabel {
  switch (row.status) {
    case "running":
      return { glyph: "◑", text: "Running" };
    case "done":
      return { glyph: "✓", text: "Done" };
    case "failed":
      return {
        glyph: "✗",
        text: typeof row.exit_code === "number" ? `Failed · exit ${row.exit_code}` : "Failed",
      };
    case "stopped":
      return { glyph: "⊘", text: row.stopped_by_owner === true ? "Stopped by you" : "Stopped" };
  }
}

function sameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  );
}

/** The time it started, with the day when that isn't `now`'s. */
export function startedLabel(iso: string, now: number): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return iso;
  }
  const time = date.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  if (sameDay(date, new Date(now))) {
    return time;
  }
  return `${date.toLocaleDateString(undefined, { month: "short", day: "numeric" })} ${time}`;
}

/** What it is for, then "background" when it outlives its turn. */
export function commandNote(row: Pick<CommandRow, "description" | "background">): string {
  const parts = [row.description ?? "", row.background ? "background" : ""];
  return parts.filter((part) => part !== "").join(" · ");
}

/** A row's key: a command id is its bot's computer's own. */
export function rowKey(row: Pick<CommandRow, "daemon_id" | "bot_id" | "id">): string {
  return `${row.daemon_id}/${row.bot_id}/${row.id}`;
}

interface Totals {
  readonly running: number;
  readonly failedToday: number;
}

/** Every computer's counts, added up. */
export function totals(summary: readonly CommandSummary[]): Totals {
  return summary.reduce<Totals>(
    (sum, s) => ({
      running: sum.running + s.running,
      failedToday: sum.failedToday + s.failed_today,
    }),
    { running: 0, failedToday: 0 },
  );
}

/** "mac 6 · win-pc 1 (not answering)". */
export function perComputer(summary: readonly CommandSummary[]): string {
  return summary
    .map((s) => `${s.computer_name} ${s.running}${s.stale === true ? " (not answering)" : ""}`)
    .join(" · ");
}

/** The name of the computer a row's bot runs on, when the summary has it. */
export function computerName(
  summary: readonly CommandSummary[],
  daemonId: string,
): string | undefined {
  return summary.find((s) => s.daemon_id === daemonId)?.computer_name;
}

import { describe, expect, it } from "vitest";
import { commandRow, commandsReply } from "../../test/commandFixtures";
import {
  commandNote,
  computerName,
  elapsed,
  perComputer,
  rowKey,
  startedLabel,
  statusLabel,
  took,
  totals,
} from "./commandsText";

const NOW = Date.parse("2025-03-04T14:20:00Z");

const time = (iso: string): string =>
  new Date(iso).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });

describe("elapsed", () => {
  it("reads like a stopwatch", () => {
    expect(elapsed("2025-01-15T10:00:00Z", "2025-01-15T10:00:12Z")).toBe("12s");
    expect(elapsed("2025-01-15T10:00:00Z", "2025-01-15T10:04:03Z")).toBe("4m 03s");
    expect(elapsed("2025-01-15T10:00:00Z", "2025-01-15T11:05:00Z")).toBe("1h 05m");
  });
});

describe("took", () => {
  it("runs to the end, or to now while running", () => {
    expect(took(commandRow({ ended_at: "2025-03-04T14:02:41Z" }), NOW)).toBe("41s");
    expect(took(commandRow(), NOW)).toBe("18m 00s");
  });
});

describe("statusLabel", () => {
  it("names each status", () => {
    expect(statusLabel(commandRow())).toEqual({ glyph: "◑", text: "Running" });
    expect(statusLabel(commandRow({ status: "done" }))).toEqual({ glyph: "✓", text: "Done" });
    expect(statusLabel(commandRow({ status: "failed", exit_code: 2 })).text).toBe(
      "Failed · exit 2",
    );
    expect(statusLabel(commandRow({ status: "failed" })).text).toBe("Failed");
    expect(statusLabel(commandRow({ status: "stopped" }))).toEqual({
      glyph: "⊘",
      text: "Stopped",
    });
    expect(statusLabel(commandRow({ status: "stopped", stopped_by_owner: true })).text).toBe(
      "Stopped by you",
    );
  });
});

describe("startedLabel", () => {
  it("is the time on now's day and adds the day otherwise", () => {
    const today = new Date(NOW);
    today.setHours(9, 5, 0, 0);
    expect(startedLabel(today.toISOString(), NOW)).toBe(time(today.toISOString()));
    const before = new Date(today);
    before.setDate(before.getDate() - 3);
    expect(startedLabel(before.toISOString(), NOW)).toMatch(
      new RegExp(`\\S+ \\d+ ${time(before.toISOString())}$`),
    );
  });

  it("leaves what isn't a date as it is", () => {
    expect(startedLabel("soon", NOW)).toBe("soon");
  });
});

describe("commandNote", () => {
  it("joins the description and background", () => {
    expect(commandNote(commandRow())).toBe("Train the RL baseline · background");
    expect(commandNote(commandRow({ background: false }))).toBe("Train the RL baseline");
    expect(commandNote(commandRow({ description: undefined }))).toBe("background");
  });
});

describe("the summary", () => {
  const summary = [
    ...commandsReply().summary,
    { daemon_id: "d-lin", computer_name: "linux", running: 0, failed_today: 2, stale: true },
  ];

  it("adds up every computer", () => {
    expect(totals(summary)).toEqual({ running: 3, failedToday: 3 });
    expect(totals([])).toEqual({ running: 0, failedToday: 0 });
  });

  it("says what runs where, and which computer didn't answer", () => {
    expect(perComputer(summary)).toBe("mac 2 · win-pc 1 · linux 0 (not answering)");
  });

  it("names a row's computer", () => {
    expect(computerName(summary, "d-win")).toBe("win-pc");
    expect(computerName(summary, "d-gone")).toBeUndefined();
  });
});

describe("rowKey", () => {
  it("tells apart the same id on two computers", () => {
    expect(rowKey(commandRow())).not.toBe(rowKey(commandRow({ daemon_id: "d-win" })));
  });
});

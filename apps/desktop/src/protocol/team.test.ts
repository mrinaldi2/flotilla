import { describe, expect, it } from "vitest";
import { bot } from "../test/fixtures";
import { TEAM_NOW, TEAM_STATUS } from "../test/teamFixtures";
import { canSleep, mirrorOn, resting, runningText, teamOn, waitedText } from "./team";

describe("the team on demand", () => {
  it("says the running count, and the queue when anyone waits", () => {
    expect(runningText(TEAM_STATUS)).toBe("4 of 6 running · 2 waiting");
    expect(runningText({ ...TEAM_STATUS, queue: [] })).toBe("4 of 6 running");
  });

  it("says how long a wake has waited", () => {
    expect(waitedText("2024-05-01T11:59:30.000Z", TEAM_NOW)).toBe("waiting under a minute");
    expect(waitedText("2024-05-01T11:56:00.000Z", TEAM_NOW)).toBe("waiting 4 min");
    expect(waitedText("2024-05-01T09:55:00.000Z", TEAM_NOW)).toBe("waiting 2 h 05 min");
    expect(waitedText("not a date", TEAM_NOW)).toBe("");
  });

  it("offers Wake to a resting bot and Sleep to an on-demand bot that is up", () => {
    expect(resting(bot({ state: "asleep" }))).toBe(true);
    expect(resting(bot({ state: "queued" }))).toBe(true);
    expect(resting(bot({ state: "ready" }))).toBe(false);
    expect(canSleep(bot({ presence: "on_demand", state: "working" }))).toBe(true);
    expect(canSleep(bot({ presence: "on_demand", state: "asleep" }))).toBe(false);
    expect(canSleep(bot({ presence: "on_demand", state: "stopped" }))).toBe(false);
    expect(canSleep(bot({ presence: "always_on", state: "ready" }))).toBe(false);
    expect(canSleep(bot({ state: "ready" }))).toBe(false);
  });

  it("reads the capabilities", () => {
    expect(teamOn({ capabilities: ["team_on_demand_v1"] })).toBe(true);
    expect(teamOn({ capabilities: [] })).toBe(false);
    expect(mirrorOn({ capabilities: ["repo_mirror_v1"] })).toBe(true);
    expect(mirrorOn({ capabilities: [] })).toBe(false);
  });
});

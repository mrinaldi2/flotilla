import { describe, expect, it } from "vitest";
import { ColumnKind } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { boards, proposals } from "../../../test/boardsFixtures";
import { lastChangeLine, leadChanges, noticeLine, openProposals } from "./lastChange";
import {
  defaultArg,
  factWord,
  kindChip,
  moverWords,
  rolesOf,
  ruleSpec,
  ruleWords,
  splitToken,
  toggledRule,
  withRoles,
} from "./rules";

describe("column rules in words", () => {
  it("pairs each kind with a glyph and a word", () => {
    expect(kindChip(ColumnKind.OPEN)).toBe("○ open");
    expect(kindChip(ColumnKind.CANCELLED)).toBe("⊘ cancelled");
    expect(kindChip(ColumnKind.UNSPECIFIED)).toBe("◌ unset");
  });

  it("words rules with their parameters", () => {
    expect(ruleWords("size_at_most:M")).toBe("Size at most M");
    expect(ruleWords("fields_filled:Goal;Steps")).toBe("Template sections filled: Goal, Steps");
    expect(ruleWords("assignee_set")).toBe("Has an assignee");
    expect(ruleWords("mystery")).toBe("mystery");
    expect(splitToken("link:pr")).toEqual({ head: "link", arg: "pr" });
  });

  it("words movers, any member when none", () => {
    expect(moverWords([])).toBe("Any member");
    expect(moverWords(["lead", "roles:dev,tester"])).toBe("The lead, Roles dev, tester");
  });

  it("switches rules and roles on and off", () => {
    expect(toggledRule(["ac_present"], "size_at_most", true, "S")).toEqual([
      "ac_present",
      "size_at_most:S",
    ]);
    expect(toggledRule(["ac_present", "size_at_most:S"], "size_at_most", false, null)).toEqual([
      "ac_present",
    ]);
    expect(withRoles(["lead"], ["dev"])).toEqual(["lead", "roles:dev"]);
    expect(withRoles(["lead", "roles:dev"], [])).toEqual(["lead"]);
    expect(rolesOf(["roles:dev,tester"])).toEqual(["dev", "tester"]);
    const size = ruleSpec("size_at_most");
    const reason = ruleSpec("reason");
    const fields = ruleSpec("fields_filled");
    expect(size && defaultArg(size)).toBe("S");
    expect(reason && defaultArg(reason)).toBeNull();
    expect(fields && defaultArg(fields)).toBe("");
    expect(factWord("pr.merged")).toBe("PR merged");
    expect(factWord("custom.fact")).toBe("custom.fact");
  });
});

const who = (actor: string): string => (actor === "bot:lead" ? "Team Lead" : "?");

describe("the lead's changes", () => {
  const now = Date.parse("2026-10-05T17:00:00Z");
  const recipes = boards()[1]?.id ?? "";

  it("lists applied changes per board and open proposals", () => {
    expect(leadChanges(proposals(), recipes).map((p) => p.id)).toEqual(["bp-1"]);
    expect(openProposals(proposals()).map((p) => p.id)).toEqual(["bp-2"]);
  });

  it("says who changed what, and when", () => {
    expect(lastChangeLine(proposals(), recipes, who, now)).toBe(
      "Last change by Team Lead, 3h ago: add column “QA” with rules ac_all_checked",
    );
    expect(lastChangeLine([], recipes, who, now)).toMatch(/Every save/);
    const [, applied] = proposals();
    expect(applied && noticeLine(applied, who, now)).toBe(
      "Team Lead: add column “QA” with rules ac_all_checked · 3h ago",
    );
  });
});

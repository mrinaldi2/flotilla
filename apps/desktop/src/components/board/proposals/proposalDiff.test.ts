import { create } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import { ColumnKind } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { BoardProposalSchema } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { RECIPES_ID, recipeColumns } from "../../../test/boardsFixtures";
import { proposalDiff } from "./proposalDiff";

function diff(change: MessageInitShape<typeof BoardProposalSchema>["change"]) {
  return proposalDiff(
    create(BoardProposalSchema, { id: "p", boardId: RECIPES_ID, summary: "it", change }),
    recipeColumns(),
  );
}

const names = (cols: readonly { readonly name: string; readonly mark?: string }[]): string[] =>
  cols.map((c) => (c.mark === undefined ? c.name : `${c.name} (${c.mark})`));

describe("a proposal before and after", () => {
  it("places an added column after the one named, with its rules", () => {
    const d = diff({
      case: "columnAdd",
      value: {
        boardId: RECIPES_ID,
        name: "Review",
        kind: ColumnKind.ACTIVE,
        after: "rb-doing",
        rules: ["link:pr"],
        movers: ["assignee"],
        wipLimit: 2,
      },
    });
    // Hidden columns (Cancelled) aren't in the strip.
    expect(names(d.before)).toEqual(["To do", "Doing", "QA", "Done"]);
    expect(names(d.after)).toEqual(["To do", "Doing", "Review (added)", "QA", "Done"]);
    expect(d.rules).toEqual(["+ adds: Has a link of kind pr", "Moved in by: The assignee"]);
    expect(d.notes).toEqual(["WIP limit 2"]);
  });

  it("lists what a new policy adds and drops", () => {
    const d = diff({
      case: "columnUpdate",
      value: {
        boardId: RECIPES_ID,
        key: "rb-qa",
        setPolicy: true,
        rules: ["ac_all_checked", "size_at_most:M"],
        movers: [],
        allowedFrom: [],
      },
    });
    expect(names(d.after)).toContain("QA (changed)");
    expect(d.rules).toEqual([
      "+ adds: Size at most M",
      "− drops: Has a link of kind pr",
      "− drops: mover: Roles tester",
      "− drops: from Doing",
    ]);
  });

  it("moves, renames, hides and removes columns", () => {
    expect(
      names(diff({ case: "columnUpdate", value: { key: "rb-done", after: "" } }).after)[0],
    ).toBe("Done (changed)");
    expect(
      names(diff({ case: "columnUpdate", value: { key: "rb-qa", name: "Test" } }).after),
    ).toContain("Test (changed)");
    expect(
      diff({ case: "columnUpdate", value: { key: "rb-qa", visible: false } }).after,
    ).toHaveLength(3);
    const removed = diff({
      case: "columnRemove",
      value: { key: "rb-qa", moveItemsTo: "rb-doing" },
    });
    expect(names(removed.after)).toContain("QA (removed)");
    expect(removed.notes).toEqual(["Its items move to Doing"]);
  });

  it("words automations, new boards and renames", () => {
    expect(
      diff({ case: "automationSet", value: { fact: "pr.merged", columnKey: "rb-done" } }).notes,
    ).toEqual(["When PR merged → move to Done"]);
    expect(diff({ case: "automationSet", value: { fact: "pr.closed" } }).notes[0]).toMatch(/stay/);
    const created = diff({ case: "boardCreate", value: { name: "Lab", preset: "simple" } });
    expect(created.before).toEqual([]);
    expect(names(created.after)).toEqual(["To do (added)", "Doing (added)", "Done (added)"]);
    expect(diff({ case: "boardUpdate", value: { name: "Kitchen" } }).notes).toEqual([
      "Renamed to Kitchen",
    ]);
    expect(diff(undefined).notes).toEqual(["it"]);
  });
});

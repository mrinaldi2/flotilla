import { describe, expect, it } from "vitest";
import { defaultColumns } from "../../../test/boardFixtures";
import { mainColumns, recipeColumns } from "../../../test/boardsFixtures";
import { flowOf } from "./drawerText";

describe("an item's path", () => {
  it("follows a custom board's own columns, Cancelled aside", () => {
    const columns = [...mainColumns(), ...recipeColumns()];
    const qa = columns.find((c) => c.key === "rb-qa");
    const flow = flowOf(columns, qa);
    expect(flow.steps.map((s) => s.word)).toEqual(["To do", "Doing", "QA", "Done"]);
    expect(flow.reached).toBe(2);
    expect(flow.next?.key).toBe("rb-done");
  });

  it("keeps the canonical steps on a release-gated board and before 0.19", () => {
    const columns = mainColumns();
    const ready = columns.find((c) => c.key === "ready");
    expect(flowOf(columns, ready)).toMatchObject({ reached: 1, next: { key: "doing" } });
    const legacy = defaultColumns();
    expect(flowOf(legacy, legacy[0]).steps[0]?.word).toBe("Inbox");
    expect(flowOf(legacy, undefined).reached).toBe(-1);
  });
});

import { describe, expect, it } from "vitest";
import { ItemType, Priority, Size } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { draftDetail } from "../../../test/boardsFixtures";
import { formOf, formProblem, updateFor } from "./itemEdit";

const item = draftDetail().item;
if (item === undefined) {
  throw new Error("the fixture has an item");
}

describe("the owner's edit of an item", () => {
  it("starts from the item and sends nothing when nothing changed", () => {
    const form = formOf(item);
    expect(form.criteria).toBe("Cups convert to grams\nGrams convert to cups");
    expect(updateFor(item, form)).toBeNull();
  });

  it("sends only what changed, with the version read", () => {
    const form = {
      ...formOf(item),
      title: "  Convert units ",
      priority: Priority.P1,
      size: Size.M,
      type: ItemType.BUG,
      labels: "kitchen, , math",
      criteria: "Cups convert to grams\n\nTablespoons too",
      description: "New",
    };
    expect(updateFor(item, form)).toEqual({
      id: "RB-20",
      expectedVersion: 6n,
      title: "Convert units",
      description: "New",
      type: ItemType.BUG,
      priority: Priority.P1,
      size: Size.M,
      labels: { values: ["kitchen", "math"] },
      acceptanceCriteria: { values: ["Cups convert to grams", "Tablespoons too"] },
    });
  });

  it("refuses a blank title and two equal criteria", () => {
    expect(formProblem({ ...formOf(item), title: " " })).toMatch(/title/);
    expect(formProblem({ ...formOf(item), criteria: "A\nA" })).toMatch(/same/);
    expect(formProblem(formOf(item))).toBeNull();
  });
});

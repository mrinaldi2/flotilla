import { create } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import { BoardSchema, LabelGroupSchema } from "../../protocol/gen/hermes/board/v1/board_pb";
import { boardLabelGroups, groupLabel, labelGroupOf, labelWord } from "./labelGroups";

describe("label groups", () => {
  it("reads a label's group and value by its prefix", () => {
    expect(labelGroupOf("area:web")).toBe("area");
    expect(labelGroupOf("urgent")).toBe("");
    expect(labelGroupOf(":web")).toBe("");
    expect(labelWord("area:web")).toBe("web");
    expect(labelWord("urgent")).toBe("urgent");
    expect(groupLabel("area", "api")).toBe("area:api");
  });

  it("offers the board's groups in order, then the groups its cards use", () => {
    const board = create(BoardSchema, {
      labelGroups: [create(LabelGroupSchema, { name: "area", values: ["web", "api"] })],
    });
    expect(
      boardLabelGroups(board, [
        ["area:db", "urgent"],
        ["team:core", "area:web"],
      ]),
    ).toEqual([
      { name: "area", values: ["web", "api", "db"] },
      { name: "team", values: ["core"] },
    ]);
    expect(boardLabelGroups(undefined, [])).toEqual([]);
  });

  it("leaves out a group the board doesn't declare when its cards use many values", () => {
    const board = create(BoardSchema, {
      labelGroups: [create(LabelGroupSchema, { name: "platform", values: ["ios"] })],
    });
    // An import's old ids (`was:B1`…), one per card: a tag, not a filter.
    const imported = Array.from({ length: 54 }, (_, i) => [`was:B${String(i)}`, "release:0.16"]);
    const offered = boardLabelGroups(board, imported).map((group) => group.name);
    expect(offered).toEqual(["platform", "release"]);
    // The board's own group keeps every value its cards use.
    const many = Array.from({ length: 12 }, (_, i) => [`platform:p${String(i)}`]);
    expect(boardLabelGroups(board, many)[0]?.values).toHaveLength(13);
  });
});

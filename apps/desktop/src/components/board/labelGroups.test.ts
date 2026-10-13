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
});

import { create } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import { DraftRevisionSchema } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { draftChanges } from "../../../test/boardsFixtures";
import { changeWords, isOwners, unseenChanges, unseenLine } from "./draftChanges";

const who = (actor: string): string => (actor === "bot:lead" ? "Team Lead" : "Builder");
const rev = (field: string, from?: string, to?: string, actor = "bot:lead") =>
  create(DraftRevisionSchema, { id: 9n, field, from, to, actor });

describe("a draft's changes", () => {
  it("counts the bots' changes after the last one seen, not the owner's", () => {
    const { revisions, seen } = draftChanges();
    const unseen = unseenChanges(revisions, seen);
    expect(unseen.map((r) => r.id)).toEqual([3n, 4n, 5n]);
    expect(unseenLine(unseen, who)).toBe("3 changes by Team Lead since you looked");
    expect(unseenLine([rev("title"), rev("size", "S", "M", "bot:b")], who)).toBe(
      "2 changes by Team Lead and Builder since you looked",
    );
    expect(isOwners("device:phone")).toBe(true);
  });

  it("words each change with a mark and what it says now", () => {
    expect(changeWords(rev("title", "A", "B"))).toMatchObject({
      field: "Title",
      mark: "~",
      summary: "A → B",
    });
    expect(changeWords(rev("size", "", "M")).summary).toBe("— → M");
    expect(changeWords(rev("description", "old", "new")).summary).toBe("Rewritten");
    expect(changeWords(rev("acceptance_criteria", "One", "One\nTwo"))).toMatchObject({
      mark: "+",
      summary: "+ Two",
    });
    expect(changeWords(rev("acceptance_criteria", "One\nTwo", "One")).mark).toBe("−");
    expect(changeWords(rev("acceptance_criteria", "A", "B")).mark).toBe("~");
    expect(changeWords(rev("acceptance_criteria", "A\nB", "B\nA")).summary).toBe("Reordered");
    expect(changeWords(rev("attachment", undefined, "spec.md")).summary).toBe("Attached spec.md");
    expect(changeWords(rev("link", "artifact:a.md", undefined))).toMatchObject({
      mark: "−",
      summary: "Removed artifact:a.md",
    });
    expect(changeWords(rev("title", "x".repeat(90), "y")).summary).toMatch(/…/);
  });
});

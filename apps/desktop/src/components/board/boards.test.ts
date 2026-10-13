import { describe, expect, it, vi } from "vitest";
import { BoardEventKind } from "../../protocol/gen/hermes/board/v1/requests_pb";
import { boardEvent, card, snapshot } from "../../test/boardFixtures";
import { boards, boardsSnapshot, MAIN_ID, RECIPES_ID } from "../../test/boardsFixtures";
import { FakeDaemon } from "../../test/fakeDaemon";
import {
  archivedBoards,
  boardOfItem,
  cardsOf,
  columnsOf,
  keyProblem,
  liveBoards,
  pickBoard,
  suggestKey,
} from "./boards";
import type { BoardStatus } from "./boardSync";
import { applyEvent, BoardSync, fromSnapshot } from "./boardSync";

const model = fromSnapshot(boardsSnapshot());

describe("a project's boards", () => {
  it("lists the default first, then by ord, and archived ones apart", () => {
    expect(liveBoards(boards()).map((b) => b.name)).toEqual(["Main", "Recipes"]);
    expect(archivedBoards(boards()).map((b) => b.name)).toEqual(["Spring cleanup"]);
  });

  it("shows the item's own board, then the one asked for, then the remembered, then the default", () => {
    expect(pickBoard(model, { item: "RB-7", asked: MAIN_ID })?.id).toBe(RECIPES_ID);
    expect(pickBoard(model, { item: "RB-20" })?.id).toBe(RECIPES_ID);
    expect(pickBoard(model, { asked: RECIPES_ID, remembered: MAIN_ID })?.id).toBe(RECIPES_ID);
    expect(pickBoard(model, { remembered: RECIPES_ID })?.id).toBe(RECIPES_ID);
    expect(pickBoard(model, { remembered: "gone" })?.id).toBe(MAIN_ID);
    // An archived board shows only when asked for.
    expect(pickBoard(model, { remembered: "b-old" })?.id).toBe(MAIN_ID);
    expect(pickBoard(model, { asked: "b-old" })?.id).toBe("b-old");
    expect(boardOfItem(model, "nobody")).toBeUndefined();
  });

  it("filters columns and cards to one board", () => {
    const recipes = pickBoard(model, { asked: RECIPES_ID });
    expect(columnsOf(model, recipes).every((c) => c.boardId === RECIPES_ID)).toBe(true);
    expect(cardsOf([...model.cards.values()], recipes)).toHaveLength(5);
  });

  it("treats a home before 0.19 as one board holding everything", () => {
    const legacy = fromSnapshot(snapshot([card({ id: "H-1" })]));
    expect(pickBoard(legacy, {})).toBeUndefined();
    expect(columnsOf(legacy, undefined)).toHaveLength(legacy.columns.length);
    expect(cardsOf([...legacy.cards.values()], undefined)).toHaveLength(1);
  });

  it("suggests a key from the name's initials and checks it", () => {
    expect(suggestKey("Recipe Box")).toBe("RB");
    expect(suggestKey("the long research lab plan")).toBe("TLRL");
    expect(suggestKey("  ")).toBe("B");
    expect(keyProblem("", boards())).toMatch(/key/);
    expect(keyProblem("rb", boards())).toMatch(/capital/);
    expect(keyProblem("RB", boards())).toBe("RB is already the key of Recipes.");
    expect(keyProblem("LAB", boards())).toBeNull();
  });
});

describe("drafts in the board model", () => {
  it("keeps a draft's card while it is one and drops it once it leaves the tray", () => {
    const changed = applyEvent(
      model,
      boardEvent({
        seq: 4n,
        kind: BoardEventKind.DRAFT_CHANGED,
        itemId: "RB-22",
        card: card({ id: "RB-22", draft: true }),
      }),
    );
    if (changed.kind !== "applied") {
      throw new Error("expected the draft push to apply");
    }
    expect(changed.model.drafts.has("RB-22")).toBe(true);
    const gone = applyEvent(
      changed.model,
      boardEvent({ seq: 5n, kind: BoardEventKind.DRAFT_CHANGED, itemId: "RB-20" }),
    );
    expect(gone.kind === "applied" && gone.model.drafts.has("RB-20")).toBe(false);
  });

  it("adopts the board a structure change answered with, unless pushes moved past it", async () => {
    const fake = new FakeDaemon().onBoard("boardWatch", () => ({
      case: "board",
      value: boardsSnapshot({ seq: 5n }),
    }));
    const seen = vi.fn<(status: BoardStatus) => void>();
    const sync = new BoardSync(fake, "p1", seen);
    sync.start();
    await vi.waitFor(() =>
      expect(seen).toHaveBeenCalledWith(expect.objectContaining({ kind: "ready" })),
    );
    seen.mockClear();
    sync.adopt(boardsSnapshot({ seq: 4n }));
    expect(seen).not.toHaveBeenCalled();
    sync.adopt(boardsSnapshot({ seq: 6n, boards: boards().slice(0, 1) }));
    const last = seen.mock.calls.at(-1)?.[0];
    expect(last?.kind === "ready" && last.model.boards).toHaveLength(1);
    sync.stop();
  });
});

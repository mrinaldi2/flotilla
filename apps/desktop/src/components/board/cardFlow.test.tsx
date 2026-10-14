import { create } from "@bufbuild/protobuf";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { MoveBlockSchema } from "../../protocol/gen/hermes/board/v1/board_pb";
import { MoveCheckSchema } from "../../protocol/gen/hermes/board/v1/requests_pb";
import { card, unmet } from "../../test/boardFixtures";
import BoardCard from "./BoardCard";
import { columnChecks, moveMenuItem, planMove, stuckLine } from "./moves";
import { PRESETS } from "./presets";

const noop = (): void => {};

describe("the owner's override on a release-gated board", () => {
  const checks = columnChecks(
    create(MoveCheckSchema, {
      itemId: "P-1",
      columns: [
        { columnKey: "ready", unmet: [unmet("dor.size", "Size it.")] },
        {
          columnKey: "doing",
          unmet: [unmet("reason.required", "This isn't a usual move; say why.")],
          ownerOverride: true,
        },
        { columnKey: "cancelled", unmet: [unmet("reason.required", "Say why.")] },
      ],
    }),
  );

  it("collects the override columns next to each column's guards", () => {
    expect([...checks.overrides]).toEqual(["doing"]);
    expect(checks.get("ready")?.[0]?.text).toBe("Size it.");
  });

  it("shows an unlisted move as Override, not ✓, and a listed one with a reason as ✓", () => {
    const doing = checks.get("doing") ?? [];
    const item = moveMenuItem(planMove(doing).kind, checks.overrides.has("doing"), doing);
    expect(item.mark).toBe("⚑");
    expect(item.reason).toBe("Override · needs a reason");
    expect(item.className).toBe(" board-move-override");
    const cancel = checks.get("cancelled") ?? [];
    expect(moveMenuItem(planMove(cancel).kind, false, cancel).mark).toBe("✓");
    const ready = checks.get("ready") ?? [];
    const refused = moveMenuItem(planMove(ready).kind, false, ready);
    expect(refused).toMatchObject({ mark: "✗", reason: "Size it.", note: "can't move: Size it." });
  });
});

describe("the card's can't-move line", () => {
  it("names the column, the first reason and how many more", () => {
    const block = create(MoveBlockSchema, {
      toColumn: "review",
      reason: "No pr is linked.",
      more: 2,
    });
    expect(stuckLine(block, "Review")).toBe("Can't move to Review: No pr is linked. (+2)");
    const one = create(MoveBlockSchema, { toColumn: "doing", reason: "Nobody is assigned." });
    expect(stuckLine(one, "Doing")).toBe("Can't move to Doing: Nobody is assigned.");
  });

  it("shows on the card, and in its accessible name, with the column's name", () => {
    const stuck = card({
      id: "P-1",
      title: "Buttons",
      columnKey: "todo",
      moveBlock: create(MoveBlockSchema, {
        toColumn: "doing",
        reason: "Nobody is assigned.",
        refused: true,
      }),
    });
    render(
      <BoardCard
        card={stuck}
        columnName="To do"
        assignee={undefined}
        onOpenMoveMenu={null}
        onDragStart={noop}
        onDragEnd={noop}
        dragging={false}
        columnNameOf={(key) => (key === "doing" ? "Doing" : key)}
      />,
    );
    const line = screen.getByText("Can't move to Doing: Nobody is assigned.");
    expect(line.closest("div")?.getAttribute("title")).toBe("The last move was refused.");
    expect(screen.getByRole("article").getAttribute("aria-label")).toContain(
      "Can't move to Doing: Nobody is assigned.",
    );
  });

  it("is absent from a card nothing stops", () => {
    render(
      <BoardCard
        card={card({ id: "P-2", columnKey: "todo" })}
        columnName="To do"
        assignee={undefined}
        onOpenMoveMenu={null}
        onDragStart={noop}
        onDragEnd={noop}
        dragging={false}
      />,
    );
    expect(screen.queryByText(/Can't move/)).toBeNull();
  });
});

describe("the first board's preset", () => {
  it("is offered with its four columns and the hidden Cancelled", () => {
    const preset = PRESETS.find((p) => p.name === "simple_review");
    expect(preset?.columns.map((c) => c.name)).toEqual([
      "To do",
      "Doing",
      "Review",
      "Done",
      "Cancelled",
    ]);
  });
});

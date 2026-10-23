import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { card } from "../../test/boardFixtures";
import BoardCard from "./BoardCard";

function show(ownerWaiting: number): void {
  render(
    <BoardCard
      card={card({ id: "CASS-057", title: "Run the combined smoke", ownerWaiting })}
      columnName="To do"
      assignee={undefined}
      onOpenMoveMenu={null}
      onDragStart={() => {}}
      onDragEnd={() => {}}
      dragging={false}
    />,
  );
}

describe("a card held on the owner (0.24.0)", () => {
  it("says it waits for you, on the card and in its name", () => {
    show(1);
    const article = screen.getByRole("article");
    expect(article).toHaveTextContent("▲ Waiting for you");
    expect(article).toHaveAccessibleName(/waiting for you/);
  });

  it("says nothing when no bot waits on you", () => {
    show(0);
    expect(screen.getByRole("article")).not.toHaveTextContent("Waiting for you");
  });
});

import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { card } from "../../test/boardFixtures";
import BoardCard from "./BoardCard";

function show(questionsOpen: number): void {
  render(
    <BoardCard
      card={card({ id: "ASL-4", title: "Sources and licensing", questionsOpen })}
      columnName="Research"
      assignee={undefined}
      onOpenMoveMenu={null}
      onDragStart={() => {}}
      onDragEnd={() => {}}
      dragging={false}
    />,
  );
}

describe("a card's open questions", () => {
  it("shows how many wait for the owner, and names them", () => {
    show(3);
    const article = screen.getByRole("article");
    expect(article).toHaveTextContent("? 3 questions for you");
    expect(article).toHaveAccessibleName(/3 questions for you$/);
  });

  it("says one question in the singular", () => {
    show(1);
    expect(screen.getByRole("article")).toHaveTextContent("1 question for you");
  });

  it("has no line without open questions", () => {
    show(0);
    expect(screen.getByRole("article")).not.toHaveTextContent("for you");
  });
});

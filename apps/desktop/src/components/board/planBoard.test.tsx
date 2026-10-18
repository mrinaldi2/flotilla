import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { card } from "../../test/boardFixtures";
import { project } from "../../test/fixtures";
import { PLAN_BOARD, PLAN_PROJECT, planNodes } from "../../test/planFixtures";
import { researchClient } from "../../test/researchFixtures";
import { filterNodes, subtrees } from "../plan/belongsTo";
import BoardView from "./BoardView";
import { matchesFilters, NO_FILTERS, parseFilters } from "./filters";

function board(withPlan = true) {
  render(
    <BoardView
      client={researchClient()}
      project={project({ id: PLAN_PROJECT, name: "PhD Game" })}
      bots={[]}
      connected
      canControl
      addToast={() => {}}
      planNodes={withPlan ? planNodes() : undefined}
    />,
  );
}

describe("an initiative's board and its plan", () => {
  it("names each card's plan node", async () => {
    board();
    const card24 = await screen.findByRole("article", { name: /^CR-24,/ });
    expect(within(card24).getByTitle("Feature: Hitstop on heavy hits")).toHaveTextContent(
      "FFeature: Hitstop on heavy hits",
    );
    const card30 = screen.getByRole("article", { name: /^CR-30,/ });
    expect(within(card30).getByTitle("Epic: Enemy attack telegraphing")).toBeVisible();
  });

  it("filters by an epic, with the cards of its features", async () => {
    const user = userEvent.setup();
    board();
    await screen.findByRole("article", { name: /^CR-24,/ });
    const nodes = screen.getByRole("group", { name: "Epic or feature" });
    expect(
      within(nodes)
        .getAllByRole("button")
        .map((b) => b.getAttribute("aria-label")),
    ).toEqual([
      "Epic: Enemy attack telegraphing",
      "Epic: Hit feedback",
      "Feature: Hitstop on heavy hits",
      "Feature: Damage numbers",
      "Epic: Combat HUD clarity",
    ]);
    await user.click(within(nodes).getByRole("button", { name: "Epic: Hit feedback" }));
    expect(screen.getByRole("status")).toHaveTextContent("Showing 6 of 9");
    expect(screen.queryByRole("article", { name: /^CR-33,/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("article", { name: /^CR-30,/ })).not.toBeInTheDocument();
    await user.click(within(nodes).getByRole("button", { name: "Epic: Combat HUD clarity" }));
    expect(screen.getByRole("article", { name: /^CR-33,/ })).toBeVisible();
  });

  it("shows no node chips or filter without the plan", async () => {
    board(false);
    const card24 = await screen.findByRole("article", { name: /^CR-24,/ });
    expect(within(card24).queryByTitle(/Feature:/)).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Epic or feature" })).not.toBeInTheDocument();
  });
});

describe("the node filter", () => {
  it("keeps a node's cards and those under it, and lets cards by without a plan", () => {
    const scope = subtrees(planNodes());
    expect(new Set(scope.get("PL-1"))).toEqual(
      new Set(["PL-1", "PL-2", "PL-3", "PL-4", "PL-5", "PL-6", "PL-7", "PL-8", "PL-9"]),
    );
    expect(new Set(scope.get("PL-5"))).toEqual(new Set(["PL-5", "PL-6", "PL-7"]));
    const filters = { ...NO_FILTERS, nodes: ["PL-5"] };
    expect(matchesFilters(card({ planNodeId: "PL-6" }), filters, scope)).toBe(true);
    expect(matchesFilters(card({ planNodeId: "PL-4" }), filters, scope)).toBe(false);
    expect(matchesFilters(card({}), filters, scope)).toBe(false);
    expect(matchesFilters(card({}), filters)).toBe(true);
    expect(parseFilters({ nodes: ["PL-5", 3] }).nodes).toEqual(["PL-5"]);
    expect(filterNodes(planNodes(), PLAN_BOARD).map((n) => n.id)).toEqual([
      "PL-4",
      "PL-5",
      "PL-6",
      "PL-7",
      "PL-8",
    ]);
  });
});

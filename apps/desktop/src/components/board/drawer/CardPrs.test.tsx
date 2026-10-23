import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { cardsDaemon } from "../../../test/cardFixtures";
import { mergedPr, PR_NOW, prDaemon, readyForYouPr, waitingPr } from "../../../test/prFixtures";
import CardPrs from "./CardPrs";
import { cardPrState } from "./cardPrText";

describe("a card's pull requests (0.24.0)", () => {
  it("lists the card's PR with where it stands, and opens it", async () => {
    const onOpenPr = vi.fn<(number: number) => void>();
    const pr = { ...waitingPr(), base: "develop", title: "Search the cards" };
    render(
      <CardPrs
        client={prDaemon([pr])}
        projectId={pr.projectId}
        connected
        itemId={pr.itemId}
        now={PR_NOW}
        onOpenPr={onOpenPr}
      />,
    );
    const open = await screen.findByRole("button", { name: `Open pull request #${pr.number}` });
    expect(screen.getByText(pr.title)).toBeInTheDocument();
    expect(screen.getByText(/^Waiting for: you/)).toBeInTheDocument();
    await userEvent.click(open);
    expect(onOpenPr).toHaveBeenCalledWith(pr.number);
  });

  it("shows nothing for another card's PR, or without pull requests", async () => {
    const pr = waitingPr();
    const { container } = render(
      <CardPrs
        client={prDaemon([pr])}
        projectId={pr.projectId}
        connected
        itemId="H-999"
        now={PR_NOW}
      />,
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(container).toBeEmptyDOMElement();
    const none = render(
      <CardPrs client={cardsDaemon()} projectId="p" connected itemId="H-1" now={PR_NOW} />,
    );
    expect(none.container).toBeEmptyDOMElement();
  });

  it("words each state in one line, naming the PR's branch", () => {
    expect(cardPrState(waitingPr(), PR_NOW).text).toMatch(/^Waiting for: you/);
    expect(cardPrState({ ...mergedPr(), base: "develop" }, PR_NOW).text).toMatch(
      /^✓ Merged into develop/,
    );
    const ready = readyForYouPr();
    const done = { ...ready, mergeable: { ...ready.mergeable, ok: true, blockers: [] } };
    expect(cardPrState({ ...done, base: "develop" } as typeof ready, PR_NOW).text).toBe(
      "Everything is in: it merges into develop next",
    );
  });
});

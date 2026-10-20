import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { recipeColumns } from "../../../test/boardsFixtures";
import { boardsClient, STORY_BOTS } from "../../../test/boardStory";
import ItemDrawer from "./ItemDrawer";
import { DRAWER_DEFAULT, loadDrawerSize } from "./useDrawerSize";

function renderDrawer() {
  return render(
    <ItemDrawer
      api={boardsClient()}
      itemId="RB-20"
      columns={recipeColumns()}
      bots={STORY_BOTS}
      canComment
      onClose={() => undefined}
    />,
  );
}

function drawer(): HTMLElement {
  const aside = document.querySelector<HTMLElement>(".item-drawer");
  if (aside === null) {
    throw new Error("no drawer");
  }
  return aside;
}

describe("the item drawer's size", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("widens over the board and back, and remembers it", async () => {
    const user = userEvent.setup();
    const { unmount } = renderDrawer();
    await user.click(await screen.findByRole("button", { name: "Widen the drawer" }));
    expect(drawer()).toHaveClass("item-drawer-wide");
    expect(screen.queryByRole("separator", { name: "Drawer width" })).toBeNull();
    unmount();

    renderDrawer();
    expect(drawer()).toHaveClass("item-drawer-wide");
    await user.click(screen.getByRole("button", { name: "Narrow the drawer" }));
    expect(drawer()).not.toHaveClass("item-drawer-wide");
    expect(loadDrawerSize().wide).toBe(false);
  });

  it("resizes from its edge with the arrow keys and resets on double-click", async () => {
    const user = userEvent.setup();
    renderDrawer();
    const edge = await screen.findByRole("separator", { name: "Drawer width" });
    expect(edge).toHaveAttribute("aria-valuenow", String(DRAWER_DEFAULT));
    edge.focus();
    await user.keyboard("{ArrowLeft}{ArrowLeft}");
    expect(edge).toHaveAttribute("aria-valuenow", String(DRAWER_DEFAULT + 80));
    expect(drawer().style.getPropertyValue("--drawer-width")).toBe(`${DRAWER_DEFAULT + 80}px`);
    expect(loadDrawerSize().width).toBe(DRAWER_DEFAULT + 80);
    await user.keyboard("{ArrowRight>10/}");
    expect(loadDrawerSize().width).toBe(360);
    await user.dblClick(edge);
    expect(edge).toHaveAttribute("aria-valuenow", String(DRAWER_DEFAULT));
  });
});

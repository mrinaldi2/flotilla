import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { recipeColumns } from "../../../test/boardsFixtures";
import { boardsClient, STORY_BOTS } from "../../../test/boardStory";
import ItemDrawer from "./ItemDrawer";

function renderDrawer() {
  const onClose = vi.fn<() => void>();
  const onStep = vi.fn<(direction: -1 | 1) => void>();
  render(
    <ItemDrawer
      api={boardsClient()}
      itemId="RB-20"
      columns={recipeColumns()}
      bots={STORY_BOTS}
      canComment
      onClose={onClose}
      onStep={onStep}
    />,
  );
  return { onClose, onStep };
}

describe("the item drawer's keys", () => {
  it("doesn't step to another item while a dialog over it has the keys", async () => {
    const user = userEvent.setup();
    const { onStep } = renderDrawer();
    await user.click(await screen.findByRole("button", { name: "Open The error full size" }));
    await screen.findByRole("dialog", { name: "import-error.png" });
    await user.keyboard("{ArrowDown}");
    expect(onStep).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "import-error.png" })).toBeInTheDocument();
  });

  it("keeps an edit when Esc is pressed in it, and cancels a link form with Esc", async () => {
    const user = userEvent.setup();
    const { onClose } = renderDrawer();
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    await user.type(screen.getByRole("textbox", { name: /^Description/ }), " more");
    await user.keyboard("{Escape}");
    expect(onClose).not.toHaveBeenCalled();
    expect(
      screen.getByRole<HTMLTextAreaElement>("textbox", { name: /^Description/ }).value,
    ).toMatch(/ more$/);
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(screen.getByRole("button", { name: "+ Link document" }));
    await user.keyboard("{Escape}");
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.queryByRole("textbox", { name: "Artifact path or URL" })).toBeNull();
  });
});

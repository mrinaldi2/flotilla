import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import MenuButton from "./MenuButton";

function renderMenu() {
  const chosen = vi.fn<(key: string) => void>();
  render(
    <MenuButton
      label="Boards"
      menuLabel="Boards"
      entries={["Main", "Recipes", "Lab"].map((name) => ({
        key: name,
        label: name,
        checked: name === "Main",
        onChoose: () => chosen(name),
      }))}
    />,
  );
  return chosen;
}

describe("a menu button", () => {
  it("opens on ↓ and moves with ↑, ↓, Home and End", async () => {
    const user = userEvent.setup();
    const chosen = renderMenu();
    screen.getByRole("button", { name: "Boards" }).focus();
    await user.keyboard("{ArrowDown}");
    const items = screen.getAllByRole("menuitemradio");
    expect(items[0]).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(items[2]).toHaveFocus();
    await user.keyboard("{ArrowDown}");
    expect(items[0]).toHaveFocus();
    await user.keyboard("{End}");
    expect(items[2]).toHaveFocus();
    await user.keyboard("{Home}{ArrowDown}{Enter}");
    expect(chosen).toHaveBeenCalledWith("Recipes");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(screen.getByRole("button", { name: "Boards" })).toHaveFocus();
  });

  it("closes on Esc, on Tab and on a press outside", async () => {
    const user = userEvent.setup();
    renderMenu();
    const button = screen.getByRole("button", { name: "Boards" });
    await user.click(button);
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).toBeNull();
    expect(button).toHaveFocus();
    await user.click(button);
    await user.keyboard("{Tab}");
    expect(screen.queryByRole("menu")).toBeNull();
    await user.click(button);
    await user.click(document.body);
    expect(screen.queryByRole("menu")).toBeNull();
  });
});

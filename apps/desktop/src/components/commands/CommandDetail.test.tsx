import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { COMMANDS_NOW, commandRow } from "../../test/commandFixtures";
import CommandDetail from "./CommandDetail";
import type { CommandRow } from "./commandsText";

function show(row: CommandRow, over: { canStop?: boolean; onStop?: () => Promise<void> } = {}) {
  const props = {
    onStop: over.onStop ?? vi.fn<() => Promise<void>>(() => Promise.resolve()),
    onOpenBot: vi.fn<() => void>(),
    onOpenItem: vi.fn<(itemId: string) => void>(),
  };
  render(
    <CommandDetail
      row={row}
      projectName="PhD"
      computer="mac"
      canStop={over.canStop ?? true}
      now={COMMANDS_NOW}
      {...props}
    />,
  );
  return props;
}

describe("CommandDetail", () => {
  it("says what runs, whose it is and what for", async () => {
    const props = show(commandRow());
    expect(screen.getByText("◑ Running · background")).toBeInTheDocument();
    expect(screen.getByText(/^started .+ · 18m 00s$/)).toBeInTheDocument();
    expect(screen.getByText("ML Engineer · PhD · mac", { exact: false })).toBeInTheDocument();
    expect(screen.getByText(/checkpoint saved/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "CASS-052" }));
    expect(props.onOpenItem).toHaveBeenCalledWith("CASS-052");
    await userEvent.click(screen.getByRole("button", { name: "Open bot" }));
    expect(props.onOpenBot).toHaveBeenCalled();
  });

  it("names a task plainly, and says when there is no output", () => {
    show(commandRow({ item_id: undefined, task_id: "t-31", output: "" }), { canStop: false });
    expect(screen.getByText(/— for task t-31/)).toBeInTheDocument();
    expect(screen.getByText("No output yet.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Stop" })).not.toBeInTheDocument();
  });

  it("asks once before stopping, and can be cancelled", async () => {
    const props = show(commandRow());
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(screen.getByText("Stop this command? The bot is told.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(props.onStop).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    await userEvent.click(screen.getByRole("button", { name: "Stop command" }));
    expect(props.onStop).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Stop this command? The bot is told.")).not.toBeInTheDocument();
  });

  it("shows a refusal in the daemon's words", async () => {
    show(commandRow(), { onStop: () => Promise.reject(new Error("not supported on Windows yet")) });
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    await userEvent.click(screen.getByRole("button", { name: "Stop command" }));
    expect(await screen.findByText("not supported on Windows yet")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop command" })).toBeEnabled();
  });
});

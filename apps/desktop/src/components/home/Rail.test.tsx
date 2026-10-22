import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Selection } from "../../app/selection";
import Rail from "./Rail";

const ENDPOINT = { host: "127.0.0.1", port: 7777 };

function renderRail(extra: Partial<Parameters<typeof Rail>[0]> = {}) {
  const onSelect = vi.fn<(next: Selection) => void>();
  render(
    <Rail
      selection={{ kind: "home" }}
      needsYou={0}
      status="connected"
      endpoint={ENDPOINT}
      canControl
      onSelect={onSelect}
      onOpenSettings={vi.fn<() => void>()}
      {...extra}
    />,
  );
  return onSelect;
}

describe("Rail", () => {
  it("has no Chat: Main chat is gone (0.24.0)", () => {
    renderRail({ commands: true, usage: true });
    expect(screen.queryByRole("button", { name: /^Chat/ })).not.toBeInTheDocument();
  });

  it("offers Commands and Usage only to a service that serves them", () => {
    renderRail();
    expect(screen.queryByRole("button", { name: /Commands/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Usage/ })).not.toBeInTheDocument();
  });

  it("opens Commands, its badge counting what runs", async () => {
    const onSelect = renderRail({ commands: true, commandsRunning: 7 });
    expect(screen.getByLabelText("7 running")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Commands/ }));
    expect(onSelect).toHaveBeenCalledWith({ kind: "commands" });
  });

  it("opens Usage, marked as the page shown", async () => {
    const onSelect = renderRail({ usage: true, selection: { kind: "usage" } });
    const usage = screen.getByRole("button", { name: /Usage/ });
    expect(usage).toHaveAttribute("aria-current", "page");
    await userEvent.click(usage);
    expect(onSelect).toHaveBeenCalledWith({ kind: "usage" });
  });
});

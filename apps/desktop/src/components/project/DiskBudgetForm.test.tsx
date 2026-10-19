import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { DiskReport } from "../../protocol/cleanup";
import type { Project } from "../../protocol/entities";
import { DISK_REPORT } from "../../test/cleanupFixtures";
import * as fx from "../../test/fixtures";
import { toastSpy } from "../../test/spies";
import { teamDaemon } from "../../test/teamFixtures";
import DiskBudgetForm from "./DiskBudgetForm";

const REPORT: DiskReport = {
  ...DISK_REPORT,
  budgets: [
    { project_id: "p1", budget_bytes: 20e9, used_bytes: 23.4e9, card_worktree_bytes: 6.1e9 },
  ],
};

function renderForm(project: Project, report: DiskReport = REPORT) {
  const client = teamDaemon()
    .onRequest("disk_report", () => ({ type: "disk_report", req_id: "1", disk_report: report }))
    .onRequest("set_project_disk_budget", (body) => ({
      type: "project",
      req_id: "1",
      project: {
        ...project,
        disk_budget_bytes: body.type === "set_project_disk_budget" ? body.bytes : null,
      },
    }));
  const onToast = toastSpy();
  render(
    <DiskBudgetForm client={client} project={project} connected canControl onToast={onToast} />,
  );
  return { client, onToast };
}

describe("a project's disk budget", () => {
  it("shows the budget with what the last disk report measured", async () => {
    renderForm(fx.project({ disk_budget_bytes: 20e9 }));
    expect(screen.getByRole("textbox", { name: /^Worktrees and build caches/ })).toHaveValue("20");
    expect(
      await screen.findByText(/Over budget: 23.4 GB of 20.0 GB, 6.1 GB of it in card worktrees/),
    ).toBeInTheDocument();
  });

  it("sets a budget in GB", async () => {
    const user = userEvent.setup();
    const { client, onToast } = renderForm(fx.project(), { ...REPORT, budgets: [] });
    const field = screen.getByRole("textbox", { name: /^Worktrees and build caches/ });
    await user.type(field, "12.5");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.requests.at(-1)?.body).toEqual({
      type: "set_project_disk_budget",
      project_id: "p1",
      bytes: 12_500_000_000,
    });
    expect(onToast).toHaveBeenCalledWith(
      "info",
      "Disk budget set",
      expect.stringContaining("12.5 GB"),
    );
  });

  it("takes the budget away when emptied", async () => {
    const user = userEvent.setup();
    const { client, onToast } = renderForm(fx.project({ disk_budget_bytes: 20e9 }));
    await user.clear(screen.getByRole("textbox", { name: /^Worktrees and build caches/ }));
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.requests.at(-1)?.body).toMatchObject({ bytes: null });
    expect(onToast).toHaveBeenCalledWith("info", "No disk budget", expect.any(String));
  });

  it("refuses what isn't a size, and reports the owner-only refusal", async () => {
    const user = userEvent.setup();
    const { client, onToast } = renderForm(fx.project());
    const field = screen.getByRole("textbox", { name: /^Worktrees and build caches/ });
    await user.type(field, "lots");
    expect(screen.getByRole("alert")).toHaveTextContent("Enter a size in GB");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    client.onRequest("set_project_disk_budget", () => {
      throw new Error("only the owner's app or a paired device sets this");
    });
    await user.clear(field);
    await user.type(field, "5");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onToast).toHaveBeenCalledWith(
      "error",
      "Couldn't set the disk budget",
      "only the owner's app or a paired device sets this",
    );
  });
});

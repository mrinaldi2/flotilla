import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ProjectTab } from "../../app/selection";
import type { AddToast } from "../../app/useToasts";
import { DaemonError } from "../../protocol/connection";
import { Role } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { BOARD_FLOW_V1 } from "../../protocol/team";
import { USAGE_V1 } from "../../protocol/usage";
import { snapshot } from "../../test/boardFixtures";
import { DASH_BOTS, quietDashboard } from "../../test/dashboardFixtures";
import { FakeDaemon } from "../../test/fakeDaemon";
import { bot, project } from "../../test/fixtures";
import { BUDGET_REACHED, BUDGET_WARNING, FLOW_ROWS, ROLE_UNHELD } from "../../test/flowRowFixtures";
import DashboardView from "./DashboardView";
import { flowRow } from "./flowRows";

function daemon(over: { capable?: boolean; owner?: boolean } = {}): FakeDaemon {
  const fake = new FakeDaemon()
    .onRequest("dashboard_get", () => ({
      type: "dashboard",
      req_id: "1",
      dashboard: { ...quietDashboard(), needs_you: [...FLOW_ROWS] },
    }))
    .onRequest("create_bot", () => ({
      type: "bot",
      req_id: "2",
      bot: bot({ id: "b9", name: "Architect" }),
    }))
    .onRequest("budget_extend", () => ({ type: "project", req_id: "3", project: project() }))
    .onBoard("roleSet", () => ({ case: "board", value: snapshot() }));
  if (over.capable !== false) {
    fake.capabilities = [...fake.capabilities, BOARD_FLOW_V1, USAGE_V1];
  }
  fake.grants = over.owner === false ? ["read", "control"] : ["read", "control", "approve"];
  return fake;
}

function renderDashboard(fake: FakeDaemon) {
  const onOpenBot = vi.fn<(botId: string) => void>();
  const onRaiseBudget = vi.fn<(projectId: string) => void>();
  const addToast = vi.fn<AddToast>();
  render(
    <DashboardView
      client={fake}
      project={project({ name: "PhD Game" })}
      bots={DASH_BOTS}
      connected
      canControl
      addToast={addToast}
      onOpenTab={vi.fn<(tab: ProjectTab) => void>()}
      onOpenBot={onOpenBot}
      onOpenDecision={vi.fn<(decisionId: string) => void>()}
      onRaiseBudget={onRaiseBudget}
    />,
  );
  return { onOpenBot, onRaiseBudget, addToast };
}

async function row(title: string): Promise<HTMLElement> {
  const item = (await screen.findByText(title)).closest("li");
  if (item === null) {
    throw new Error(`no row for ${title}`);
  }
  return item;
}

describe("Needs you's 0.24 rows", () => {
  it("creates a bot with the role nobody holds, then opens it", async () => {
    const user = userEvent.setup();
    const fake = daemon();
    const { onOpenBot } = renderDashboard(fake);
    const unheld = await row(ROLE_UNHELD.title);
    expect(unheld).toHaveTextContent("Its cards stay with whoever moved them");
    await user.click(within(unheld).getByRole("button", { name: "Create a bot with Architect" }));
    await waitFor(() => {
      expect(onOpenBot).toHaveBeenCalledWith("b9");
    });
    expect(fake.requests.map((r) => r.body)).toContainEqual({
      type: "create_bot",
      project_id: "p1",
      name: "Architect",
    });
    expect(fake.boardCalls).toMatchObject([
      { case: "roleSet", value: { projectId: "p1", bot: "b9", role: Role.ARCHITECT } },
    ]);
  });

  it("raises a budget near its end in the project's Settings", async () => {
    const user = userEvent.setup();
    const { onRaiseBudget } = renderDashboard(daemon());
    const warning = await row(BUDGET_WARNING.title);
    expect(warning).toHaveTextContent("At 100% its on-demand bots stop waking");
    await user.click(within(warning).getByRole("button", { name: /^Raise budget/ }));
    expect(onRaiseBudget).toHaveBeenCalledWith("p1");
  });

  it("extends a used-up budget for today, or raises it", async () => {
    const user = userEvent.setup();
    const fake = daemon();
    const { onRaiseBudget, addToast } = renderDashboard(fake);
    const reached = await row(BUDGET_REACHED.title);
    expect(reached).toHaveTextContent(
      "Its on-demand bots wait as Budget reached; the lead keeps going",
    );
    await user.click(within(reached).getByRole("button", { name: /^Extend today/ }));
    await waitFor(() => {
      expect(addToast).toHaveBeenCalledWith(
        "info",
        "Budget extended for today",
        expect.any(String),
      );
    });
    expect(fake.requests.map((r) => r.body)).toContainEqual({
      type: "budget_extend",
      project_id: "p2",
    });
    await user.click(within(reached).getByRole("button", { name: /^Raise budget/ }));
    expect(onRaiseBudget).toHaveBeenCalledWith("p2");
  });

  it("says why an extension was refused", async () => {
    const user = userEvent.setup();
    const fake = daemon().onRequest("budget_extend", () => {
      throw new DaemonError("conflict", "the project has no budget to extend");
    });
    const { addToast } = renderDashboard(fake);
    const reached = await row(BUDGET_REACHED.title);
    await user.click(within(reached).getByRole("button", { name: /^Extend today/ }));
    await waitFor(() => {
      expect(addToast).toHaveBeenCalledWith(
        "error",
        "Couldn't extend the budget",
        "the project has no budget to extend",
      );
    });
  });

  it("leaves roles and budgets to the owner", async () => {
    renderDashboard(daemon({ owner: false }));
    const unheld = await row(ROLE_UNHELD.title);
    const create = within(unheld).getByRole("button", { name: "Create a bot with Architect" });
    expect(create).toBeDisabled();
    expect(create).toHaveAttribute("title", expect.stringMatching(/Only the owner/));
    const reached = await row(BUDGET_REACHED.title);
    expect(within(reached).getByRole("button", { name: /^Extend today/ })).toBeDisabled();
    // Opening Settings is for anyone; the form there is the owner's.
    expect(within(reached).getByRole("button", { name: /^Raise budget/ })).toBeEnabled();
  });

  it("offers nothing a service before 0.24 can't do", async () => {
    renderDashboard(daemon({ capable: false }));
    for (const r of FLOW_ROWS) {
      // oxlint-disable-next-line no-await-in-loop
      expect(within(await row(r.title)).queryByRole("button")).toBeNull();
    }
  });
});

describe("a 0.24 row's words", () => {
  it("offers no bot for a role this app doesn't know, nor a raise without a project", () => {
    const actions = { onCreateRoleBot: vi.fn<(role: Role) => void>(), canApprove: true };
    expect(flowRow({ ...ROLE_UNHELD, role: "astronaut" }, actions)?.action).toBeUndefined();
    const warning = flowRow(
      { ...BUDGET_WARNING, project_id: undefined },
      { onRaiseBudget: vi.fn<(id: string) => void>() },
    );
    expect(warning?.action).toBeUndefined();
    expect(flowRow({ ...ROLE_UNHELD, kind: "disk_low" }, actions)).toBeUndefined();
  });

  it("says a bot is being created, and an extension is on its way", () => {
    expect(flowRow(ROLE_UNHELD, { creatingRole: Role.ARCHITECT, canApprove: true })?.meta).toBe(
      "Creating the bot…",
    );
    const reached = flowRow(BUDGET_REACHED, {
      onExtendBudget: vi.fn<(id: string) => void>(),
      extending: "p2",
      canApprove: true,
    });
    expect(reached?.meta).toBe("Extending it for today…");
    expect(reached?.disabled).toBe(true);
  });
});

import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ProjectTab } from "../../app/selection";
import type { AddToast } from "../../app/useToasts";
import type { AttentionRowJson } from "../../protocol/dashboard";
import { DASH_BOTS, dashboard } from "../../test/dashboardFixtures";
import { FakeDaemon } from "../../test/fakeDaemon";
import { project } from "../../test/fixtures";
import DashboardView from "./DashboardView";

/** The rows a 0.21 daemon raises for plan gates (attention/plan.rs). */
const ROWS: readonly AttentionRowJson[] = [
  {
    kind: "plan_gate",
    id: "plan_gate:d-mac:PL-8",
    title: "Approve the HLD of Combat HUD clarity?",
    plan_node_id: "PL-8",
  },
  {
    kind: "plan_gate",
    id: "plan_gate:d-mac:PL-12",
    title: "Accept the proposed initiative Boss fights?",
    plan_node_id: "PL-12",
  },
  {
    kind: "plan_gate",
    id: "plan_gate:d-mac:PL-9",
    title: "Skip research for Combat onboarding?",
    plan_node_id: "PL-9",
  },
];

describe("plan gates in Needs you", () => {
  it("opens each row's own node on its gate", async () => {
    const user = userEvent.setup();
    const data = dashboard();
    const fake = new FakeDaemon().onRequest("dashboard_get", () => ({
      type: "dashboard",
      req_id: "1",
      dashboard: { ...data, needs_you: [...ROWS, ...data.needs_you] },
    }));
    const onOpenPlanGate = vi.fn<(nodeId: string) => void>();
    render(
      <DashboardView
        client={fake}
        project={project({ name: "PhD Game" })}
        bots={DASH_BOTS}
        connected
        canControl
        addToast={vi.fn<AddToast>()}
        onOpenTab={vi.fn<(tab: ProjectTab) => void>()}
        onOpenBot={vi.fn<(botId: string) => void>()}
        onOpenDecision={vi.fn<(decisionId: string) => void>()}
        onOpenPlanGate={onOpenPlanGate}
      />,
    );
    for (const row of ROWS) {
      // oxlint-disable-next-line no-await-in-loop
      const item = (await screen.findByText(row.title)).closest("li");
      if (item === null) {
        throw new Error(`no row for ${row.title}`);
      }
      // One row after another, as the owner clicks them.
      // oxlint-disable-next-line no-await-in-loop
      await user.click(within(item).getByRole("button", { name: /Review the gate/ }));
    }
    expect(onOpenPlanGate.mock.calls).toEqual([["PL-8"], ["PL-12"], ["PL-9"]]);
  });
});

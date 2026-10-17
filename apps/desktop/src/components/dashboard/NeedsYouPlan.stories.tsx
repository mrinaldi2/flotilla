import type { Story } from "@ladle/react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { ProjectTab } from "../../app/selection";
import type { AddToast } from "../../app/useToasts";
import type { AttentionRowJson } from "../../protocol/dashboard";
import { DISK_REPORT } from "../../test/cleanupFixtures";
import { DASH_BOTS, dashboard } from "../../test/dashboardFixtures";
import { FakeDaemon } from "../../test/fakeDaemon";
import { project } from "../../test/fixtures";
import { flowMetrics } from "../../test/flowFixtures";
import DashboardView from "./DashboardView";

const noTab = (_tab: ProjectTab): void => {};
const noop = (): void => {};
const noToast: AddToast = () => {};

/** Plan gates (0.21.0): an HLD and a proposed initiative waiting for the owner. */
const PLAN_ROWS: readonly AttentionRowJson[] = [
  {
    kind: "plan_gate",
    id: "plan_gate:mac:PL-8",
    title: "Approve the HLD of Combat HUD clarity?",
    plan_node_id: "PL-8",
  },
  {
    kind: "plan_gate",
    id: "plan_gate:mac:PL-12",
    title: "Accept the proposed initiative Boss fights?",
    plan_node_id: "PL-12",
  },
];

function Frame(): ReactElement {
  const [client] = useState(() => {
    const data = dashboard();
    return new FakeDaemon()
      .onRequest("dashboard_get", () => ({
        type: "dashboard",
        req_id: "1",
        dashboard: { ...data, needs_you: [...PLAN_ROWS, ...data.needs_you] },
      }))
      .onRequest("metrics_get", () => ({
        type: "metrics",
        req_id: "2",
        metrics: flowMetrics(),
        note: null,
      }))
      .onRequest("disk_report", () => ({
        type: "disk_report",
        req_id: "3",
        disk_report: DISK_REPORT,
      }));
  });
  return (
    <div className="main" style={{ height: 1100 }}>
      <DashboardView
        client={client}
        project={project({ id: "p1", name: "PhD Game" })}
        bots={DASH_BOTS}
        connected
        canControl
        addToast={noToast}
        onOpenTab={noTab}
        onOpenBot={noop}
        onOpenDecision={noop}
        onOpenPlanGate={noop}
      />
    </div>
  );
}

/** Each plan gate in Needs you opens its node's gate in the Plan tab. */
export const PlanGates: Story = () => <Frame />;

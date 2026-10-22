import type { Story } from "@ladle/react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { ProjectTab } from "../../app/selection";
import type { AddToast } from "../../app/useToasts";
import { BOARD_FLOW_V1 } from "../../protocol/team";
import { USAGE_V1 } from "../../protocol/usage";
import { DISK_REPORT } from "../../test/cleanupFixtures";
import { DASH_BOTS, dashboard } from "../../test/dashboardFixtures";
import { FakeDaemon } from "../../test/fakeDaemon";
import { project } from "../../test/fixtures";
import { flowMetrics } from "../../test/flowFixtures";
import { FLOW_ROWS } from "../../test/flowRowFixtures";
import DashboardView from "./DashboardView";

const noTab = (_tab: ProjectTab): void => {};
const noop = (): void => {};
const noToast: AddToast = () => {};

function Frame(): ReactElement {
  const [client] = useState(() => {
    const data = dashboard();
    const fake = new FakeDaemon()
      .onRequest("dashboard_get", () => ({
        type: "dashboard",
        req_id: "1",
        dashboard: { ...data, needs_you: [...FLOW_ROWS, ...data.needs_you] },
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
    fake.capabilities = [...fake.capabilities, BOARD_FLOW_V1, USAGE_V1];
    fake.grants = ["read", "control", "approve"];
    return fake;
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
        onRaiseBudget={noop}
      />
    </div>
  );
}

/**
 * The board runs the SDLC and budgets (0.24.0): a column's role nobody
 * holds offers Create a bot; a budget near its end, Raise budget…; a used-up
 * one, Extend today or Raise budget….
 */
export const RolesAndBudgets: Story = () => <Frame />;

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

/** A bot changed a draft since the owner looked (0.19.0 K5): "✎ … Review". */
const DRAFT_ROW: AttentionRowJson = {
  kind: "draft_updated",
  id: "draft_updated:RB-20",
  title: "Team Lead updated draft RB-20: Import recipes from a URL",
  item_id: "RB-20",
};

function Frame({ light }: { readonly light?: boolean }): ReactElement {
  const [client] = useState(() => {
    const data = dashboard();
    return new FakeDaemon()
      .onRequest("dashboard_get", () => ({
        type: "dashboard",
        req_id: "1",
        dashboard: { ...data, needs_you: [DRAFT_ROW, ...data.needs_you] },
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
    <div className={`main ${light ? "theme-light" : ""}`} style={{ height: 1100 }}>
      <DashboardView
        client={client}
        project={project({ id: "p1", name: "Flotilla" })}
        bots={DASH_BOTS}
        connected
        canControl
        addToast={noToast}
        onOpenTab={noTab}
        onOpenBot={noop}
        onOpenDecision={noop}
      />
    </div>
  );
}

export const DraftUpdated: Story = () => <Frame />;

export const DraftUpdatedLight: Story = () => <Frame light />;

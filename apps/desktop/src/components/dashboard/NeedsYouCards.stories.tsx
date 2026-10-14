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

/** The board keeper's rows (0.19.0): cards waiting on the owner, each opening its card. */
const CARD_ROWS: readonly AttentionRowJson[] = [
  {
    kind: "card_waiting",
    id: "card_waiting:mac:RB-12:review",
    title: "RB-12 waits for your review: Button variants",
    item_id: "RB-12",
  },
  {
    kind: "card_waiting",
    id: "card_waiting:mac:RB-14:quiet",
    title: "RB-14 is in Doing and nothing has happened for 6 h: Checkout page",
    item_id: "RB-14",
  },
  {
    kind: "card_waiting",
    id: "card_waiting:mac:RB-15:blocked",
    title: "RB-15 can't move to Done: The mover can't be its assignee, now or before.",
    item_id: "RB-15",
  },
];

function Frame({ light }: { readonly light?: boolean }): ReactElement {
  const [client] = useState(() => {
    const data = dashboard();
    return new FakeDaemon()
      .onRequest("dashboard_get", () => ({
        type: "dashboard",
        req_id: "1",
        dashboard: { ...data, needs_you: [...CARD_ROWS, ...data.needs_you] },
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
        project={project({ id: "p1", name: "The Hermes" })}
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

export const CardsWaiting: Story = () => <Frame />;

export const CardsWaitingLight: Story = () => <Frame light />;

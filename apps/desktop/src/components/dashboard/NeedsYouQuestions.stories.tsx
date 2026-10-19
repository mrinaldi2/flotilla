import type { Story } from "@ladle/react";
import { useRef, useState } from "react";
import type { ReactElement } from "react";
import type { ProjectTab } from "../../app/selection";
import type { AddToast } from "../../app/useToasts";
import { DISK_REPORT } from "../../test/cleanupFixtures";
import { DASH_BOTS, dashboard } from "../../test/dashboardFixtures";
import { project } from "../../test/fixtures";
import { flowMetrics } from "../../test/flowFixtures";
import {
  calendarQuestion,
  QUESTION_ROWS,
  questionsClient,
  saintsQuestion,
} from "../../test/questionFixtures";
import { useClickOnce } from "../../test/storyClick";
import DashboardView from "./DashboardView";

const noTab = (_tab: ProjectTab): void => {};
const noop = (): void => {};
const noToast: AddToast = () => {};

function Frame(props: { readonly click?: string }): ReactElement {
  const box = useRef<HTMLDivElement>(null);
  const [client] = useState(() => {
    const data = dashboard();
    return questionsClient([saintsQuestion(), calendarQuestion()])
      .onRequest("dashboard_get", () => ({
        type: "dashboard",
        req_id: "1",
        dashboard: { ...data, needs_you: [...QUESTION_ROWS, ...data.needs_you] },
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
  useClickOnce(box, props.click);
  return (
    <div className="main" ref={box} style={{ height: 1100 }}>
      <DashboardView
        client={client}
        project={project({ id: "p1", name: "Saints" })}
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

/** One row per question a bot asked; the blocking one ranks with the gates. */
export const QuestionRows: Story = () => <Frame />;
/** Answer opens the question under its row, answered there. */
export const AnswerInline: Story = () => <Frame click="Answer" />;

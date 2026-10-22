// The Plan tab inside its project window (0.21.0), for stories and tests: a
// fake Flotilla service that serves the plan of a game project.

import { useState } from "react";
import type { ReactElement } from "react";
import type { ProjectTab } from "../app/selection";
import PlanTab from "../components/plan/PlanTab";
import type { PlanViewProps } from "../components/plan/PlanView";
import { usePlan } from "../components/plan/usePlan";
import ProjectWindow from "../components/project/ProjectWindow";
import type { FakeDaemon } from "./fakeDaemon";
import { bot, project } from "./fixtures";
import { PLAN_PROJECT, planClient } from "./planFixtures";

const PLAN_BOTS = [
  bot({ id: "lead", name: "Game Lead", avatar: "color:#f59e0b" }),
  bot({ id: "b1", name: "Designer", avatar: "color:#f9a8d4" }),
  bot({ id: "b2", name: "Gameplay Dev", avatar: "color:#4ade80" }),
];

const noTab = (_tab: ProjectTab): void => {};

export function PlanWindow(props: {
  readonly client?: FakeDaemon;
  readonly openNode?: string;
  readonly openGate?: boolean;
  readonly initial?: PlanViewProps["initial"];
  readonly onOpenItem?: (id: string) => void;
  readonly height?: number;
}): ReactElement {
  const [client] = useState(() => props.client ?? planClient());
  const p = project({ id: PLAN_PROJECT, name: "PhD Game", lead_bot_id: "lead" });
  const plan = usePlan(client, p.id, true);
  return (
    <div className="main" style={{ height: props.height ?? 700 }}>
      <ProjectWindow
        project={p}
        botCount={PLAN_BOTS.length}
        tab="plan"
        plan
        prs
        onSelectTab={noTab}
      >
        <PlanTab
          client={client}
          project={p}
          bots={PLAN_BOTS}
          connected
          plan={plan}
          addToast={() => {}}
          openNode={props.openNode}
          openGate={props.openGate}
          onOpenItem={props.onOpenItem}
          initial={props.initial}
        />
      </ProjectWindow>
    </div>
  );
}

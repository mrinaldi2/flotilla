// The Plan tab's states (0.21.0): an older service, the tree loading or
// failing, planning off for the project, or the plan itself. While it shows,
// it watches the project so `plan_change` pushes keep the tree live.

import { useEffect } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { Bot, Project } from "../../protocol/entities";
import type { UsePlan } from "./usePlan";
import { usePlanWatch } from "./usePlan";
import PlanView from "./PlanView";
import type { PlanViewProps } from "./PlanView";

interface PlanTabProps {
  readonly client: DaemonApi;
  readonly project: Project;
  readonly bots: readonly Bot[];
  readonly connected: boolean;
  readonly plan: UsePlan;
  readonly openNode?: string;
  readonly openGate?: boolean;
  readonly onOpenItem?: (id: string) => void;
  readonly onOpenBoard?: (boardId: string) => void;
  readonly initial?: PlanViewProps["initial"];
}

function Notice(props: {
  readonly title: string;
  readonly text?: string;
  readonly retry?: () => void;
}): ReactElement {
  return (
    <div className="empty-pane">
      <div className="empty-state">
        <h1>{props.title}</h1>
        {props.text === undefined ? null : <p>{props.text}</p>}
        {props.retry === undefined ? null : (
          <button type="button" className="btn btn-small" onClick={props.retry}>
            Try again
          </button>
        )}
      </div>
    </div>
  );
}

export default function PlanTab(props: PlanTabProps): ReactElement {
  const { client, project, connected, plan } = props;
  const { status, refresh } = plan;
  usePlanWatch(client, project.id, connected && status.kind !== "unsupported");
  // The tab reads the tree afresh when it opens: pushes only reach a watch.
  useEffect(() => {
    refresh();
  }, [refresh]);
  switch (status.kind) {
    case "unsupported":
      return <Notice title="The plan needs a newer Flotilla service" />;
    case "loading":
      return (
        <Notice title={connected ? "Loading the plan…" : "Waiting for the Flotilla service…"} />
      );
    case "failed":
      return <Notice title="Couldn't load the plan" text={status.message} retry={refresh} />;
    case "ready":
      break;
  }
  if (!status.tree.enabled) {
    return (
      <Notice
        title="Planning is off for this project"
        text="Goals, initiatives, epics and features live here once planning is on. It needs the project's repository, where their documents are kept."
      />
    );
  }
  const lead = props.bots.find((b) => b.id === project.lead_bot_id);
  return (
    <PlanView
      api={client}
      projectId={project.id}
      tree={status.tree}
      bots={props.bots}
      leadName={lead?.name}
      canRule={client.hasGrant("approve")}
      canCreate={client.hasGrant("control")}
      onChanged={plan.apply}
      openNode={props.openNode}
      openGate={props.openGate}
      onOpenItem={props.onOpenItem}
      onOpenBoard={props.onOpenBoard}
      initial={props.initial}
    />
  );
}

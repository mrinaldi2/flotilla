// The owner's switch for a project's plan (0.21.0, `plan_enable`): turning
// it on needs the project's repository, where the plan's documents live;
// turning it off hides the plan and keeps it. The daemon tells the
// project's bots either way. With planning on, the owner also sets whether
// research waits for their Approve findings (0.24.0, `research_gate_v1`).

import { useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { BoardApi } from "../../protocol/board";
import type { ProjectRepo } from "../../protocol/entities";
import { boardCall } from "../../protocol/board";
import { RESEARCH_GATE_V1 } from "../../protocol/plan";
import { errText } from "../../util";
import type { UsePlan } from "./usePlan";
import { planOn } from "./usePlan";

interface PlanningSettingsProps {
  /** The board's requests, and the grants: only the owner turns the research gate. */
  readonly api: BoardApi & Pick<DaemonApi, "hasGrant">;
  readonly projectId: string;
  /** The project's plan; none (or a daemon without it) hides the panel. */
  readonly plan?: UsePlan;
  /** The project's repository, if set. */
  readonly repo?: ProjectRepo | null;
  readonly connected: boolean;
  readonly canControl: boolean;
}

export default function PlanningSettings(props: PlanningSettingsProps): ReactElement | null {
  const { plan } = props;
  if (plan === undefined || plan.status.kind === "unsupported") {
    return null;
  }
  return <Panel {...props} plan={plan} hasRepo={props.repo !== null && props.repo !== undefined} />;
}

/** "Research waits for my approval": the project's research gate, the owner's to turn. */
function ResearchGate(
  props: PlanningSettingsProps & { readonly plan: UsePlan },
): ReactElement | null {
  const { api, plan } = props;
  if (plan.status.kind !== "ready" || !plan.status.tree.enabled) {
    return null;
  }
  if (!api.capabilities.includes(RESEARCH_GATE_V1)) {
    return null;
  }
  return <GateSwitch {...props} gate={plan.status.tree.researchGate} />;
}

function GateSwitch(
  props: PlanningSettingsProps & { readonly plan: UsePlan; readonly gate: boolean },
): ReactElement {
  const { api, projectId, plan } = props;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const flip = async (next: boolean): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      const value = { projectId, enabled: true, researchGate: next };
      await boardCall(api, { case: "planEnable", value }, "plan");
      plan.refresh();
    } catch (failure) {
      setError(errText(failure));
    } finally {
      setBusy(false);
    }
  };
  const allowed = props.connected && props.canControl && api.hasGrant("approve");
  return (
    <>
      <label className="checkbox-row">
        <input
          type="checkbox"
          checked={props.gate}
          disabled={!allowed || busy}
          onChange={(e) => void flip(e.target.checked)}
        />
        Research waits for my approval
      </label>
      <p className="field-hint">
        When every spike is done, research waits for your Approve findings or Send back. Off, it
        closes by itself.
      </p>
      {error === null ? null : (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
    </>
  );
}

function Panel(
  props: PlanningSettingsProps & { readonly plan: UsePlan; readonly hasRepo: boolean },
): ReactElement {
  const { api, projectId, plan, hasRepo, connected, canControl } = props;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const on = planOn(plan.status);
  const toggle = async (): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      await boardCall(api, { case: "planEnable", value: { projectId, enabled: !on } }, "plan");
      plan.refresh();
    } catch (failure) {
      setError(errText(failure));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="panel">
      <h3 className="panel-title">Planning</h3>
      <p className="field-hint">
        Goals, initiatives, epics and features, each with its documents in the project&apos;s
        repository and approved by you. Each initiative gets its own board; features are never
        cards. Turning it off hides the plan and keeps it.
      </p>
      {!hasRepo && !on ? (
        <p className="field-hint">Set the project&apos;s repository above first.</p>
      ) : null}
      {error === null ? null : (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
      <div className="panel-actions">
        <button
          type="button"
          className={on ? "btn" : "btn btn-primary"}
          disabled={!connected || !canControl || busy || (!on && !hasRepo)}
          onClick={() => {
            void toggle();
          }}
        >
          {on ? "Turn planning off" : "Turn planning on"}
        </button>
      </div>
      <ResearchGate {...props} />
    </div>
  );
}

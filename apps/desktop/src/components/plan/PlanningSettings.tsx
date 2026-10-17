// The owner's switch for a project's plan (0.21.0, `plan_enable`): turning
// it on needs the project's repository, where the plan's documents live;
// turning it off hides the plan and keeps it. The daemon tells the
// project's bots either way.

import { useState } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import { boardCall } from "../../protocol/board";
import { errText } from "../../util";
import type { UsePlan } from "./usePlan";
import { planOn } from "./usePlan";

interface PlanningSettingsProps {
  readonly api: BoardApi;
  readonly projectId: string;
  readonly plan: UsePlan;
  /** The project has a repository set. */
  readonly hasRepo: boolean;
  readonly connected: boolean;
  readonly canControl: boolean;
}

export default function PlanningSettings(props: PlanningSettingsProps): ReactElement | null {
  const { api, projectId, plan, hasRepo, connected, canControl } = props;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (plan.status.kind === "unsupported") {
    return null;
  }
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
    </div>
  );
}

// A node's stages in order (0.21.0): passed, current, to come, the gates
// marked ✓. Each step says where it stands in words, never colour alone.

import type { ReactElement } from "react";
import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { PlanResearchState, PlanStage } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { isGateStage, stagesOf, stageWord } from "./planText";

type StepState = "past" | "now" | "next";

interface Step {
  readonly stage: PlanStage;
  readonly state: StepState;
  readonly sub: string;
}

/** The line under the current step, and under a passed Research step. */
function stepSub(node: PlanNode, stage: PlanStage, state: StepState): string {
  if (stage === PlanStage.RESEARCH && node.research !== undefined && state !== "next") {
    const { research } = node;
    if (research.state === PlanResearchState.SKIPPED) {
      return "Skipped";
    }
    const n = research.spikes.length;
    const spikes = `${n} ${n === 1 ? "spike" : "spikes"}`;
    return research.state === PlanResearchState.CLOSED ? `Closed · ${spikes}` : `Open · ${spikes}`;
  }
  if (state !== "now") {
    return "";
  }
  if (node.gate !== undefined) {
    return "Waiting for you";
  }
  return isGateStage(stage) ? "Being written" : "";
}

/** The node's stages, each passed, current or to come. */
export function stepsOf(node: PlanNode): readonly Step[] {
  const stages = stagesOf(node.kind);
  const reached = stages.indexOf(node.stage);
  return stages.map((stage, index) => {
    const state: StepState =
      reached < 0 || index > reached ? "next" : index < reached ? "past" : "now";
    return { stage, state, sub: stepSub(node, stage, state) };
  });
}

export default function PlanStepper({ node }: { readonly node: PlanNode }): ReactElement {
  const steps = stepsOf(node);
  const off =
    node.stage === PlanStage.DROPPED
      ? "Dropped"
      : node.stage === PlanStage.PROPOSED
        ? "Proposed: waiting for you to accept it"
        : null;
  return (
    <div className="plan-stepper-wrap">
      <ol className="plan-stepper" aria-label="Stages">
        {steps.map((s) => (
          <li
            key={s.stage}
            className={`plan-step plan-step-${s.state}`}
            aria-current={s.state === "now" ? "step" : undefined}
          >
            <span className="plan-step-bar" aria-hidden="true" />
            <span className="plan-step-name">
              {isGateStage(s.stage) ? (
                <span className="plan-step-gate" title="A gate you approve">
                  <span aria-hidden="true">✓</span>
                  <span className="visually-hidden">Gate: </span>
                </span>
              ) : null}
              {stageWord(s.stage)}
              <span className="visually-hidden">
                {s.state === "past" ? " (passed)" : s.state === "now" ? " (now)" : ""}
              </span>
            </span>
            {s.sub === "" ? null : <span className="plan-step-sub">{s.sub}</span>}
          </li>
        ))}
      </ol>
      {off === null ? null : <p className="plan-line">{off}</p>}
    </div>
  );
}

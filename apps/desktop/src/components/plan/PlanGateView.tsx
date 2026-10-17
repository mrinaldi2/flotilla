// A gate waiting for the owner (0.21.0, canvas D2): the submitted document
// read in place at its commit, beside the ruling. A proposed initiative and
// a request to skip research have no document: their summary stands in.

import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { PlanGate, PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import Markdown from "../control/Markdown";
import PlanDocReader from "./PlanDocReader";
import PlanGatePanel from "./PlanGatePanel";
import { stageWord } from "./planText";

interface PlanGateViewProps {
  readonly api: BoardApi;
  readonly projectId: string;
  readonly node: PlanNode;
  readonly gate: PlanGate;
  readonly ancestors: readonly PlanNode[];
  readonly canRule: boolean;
  readonly who: (actor: string) => string;
  readonly onBack: () => void;
  readonly onRuled: (node: PlanNode) => void;
}

export default function PlanGateView(props: PlanGateViewProps): ReactElement {
  const { node, gate } = props;
  const path = gate.docPath;
  const approved = node.docs.find((d) => d.path === path)?.approvedSha;
  return (
    <div className="plan-gate">
      <header className="plan-gate-bar">
        <button type="button" className="plan-link" onClick={props.onBack}>
          ‹ Plan
        </button>
        {props.ancestors.length === 0 ? null : (
          <span className="plan-dim">{`${props.ancestors.map((a) => a.title).join(" › ")} ›`}</span>
        )}
        <strong>{node.title}</strong>
        <span className="plan-chip plan-chip-you">{`${stageWord(gate.stage)} · waiting for ${props.canRule ? "you" : "the owner"}`}</span>
      </header>
      <div className="plan-gate-body">
        <div className="plan-gate-doc">
          {path === undefined ? (
            <div className="plan-summary">
              {node.summary.trim() === "" ? (
                <p className="plan-empty">No summary.</p>
              ) : (
                <Markdown>{node.summary}</Markdown>
              )}
            </div>
          ) : (
            <PlanDocReader
              key={`${path}@${gate.sha ?? ""}`}
              api={props.api}
              projectId={props.projectId}
              path={path}
              sha={gate.sha}
              approvedSha={approved}
            />
          )}
          {gate.prNumber === undefined ? null : (
            <p className="plan-dim">{`In pull request #${gate.prNumber}`}</p>
          )}
        </div>
        <PlanGatePanel
          key={`${node.id}-${gate.round}`}
          api={props.api}
          node={node}
          gate={gate}
          canRule={props.canRule}
          who={props.who}
          onRuled={props.onRuled}
        />
      </div>
    </div>
  );
}

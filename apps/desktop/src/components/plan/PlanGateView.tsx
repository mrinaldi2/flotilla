// A gate waiting for the owner (0.21.0, canvas D2): the node's steps, the
// submitted document read in place at its commit, beside the ruling.
// Research waiting for the owner (0.24.0) shows its spikes' documents. A
// proposed initiative and a request to skip research have no document:
// their summary stands in.

import type { ReactElement, ReactNode } from "react";
import type { BoardApi } from "../../protocol/board";
import type { ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { PlanGate, PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import Markdown from "../control/Markdown";
import PlanDocReader from "./PlanDocReader";
import PlanGateDocs from "./PlanGateDocs";
import PlanGatePanel from "./PlanGatePanel";
import PlanStepper from "./PlanStepper";
import PaneEdge, { GATE_PANEL_WIDTH } from "./PaneEdge";
import { usePaneWidth } from "../../hooks/usePaneWidth";
import { researchAwaits, stageWord } from "./planText";
import { usePlanNode } from "./usePlanNode";

/** What the gate shows besides the ruling, for the node and its cards: its push requests. */
export type GateExtras = (node: PlanNode, itemIds: readonly string[]) => ReactNode;

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
  readonly extras?: GateExtras;
}

/** The node's cards: its spikes, its documents' cards and the cards read with it. */
function gateItemIds(node: PlanNode, cards: readonly ItemCard[]): readonly string[] {
  const docs = node.docs.flatMap((d) => (d.itemId === undefined ? [] : [d.itemId]));
  return [...new Set([...(node.research?.spikes ?? []), ...docs, ...cards.map((c) => c.id)])];
}

function Summary({ node }: { readonly node: PlanNode }): ReactElement {
  return (
    <div className="plan-summary">
      {node.summary.trim() === "" ? (
        <p className="plan-empty">No summary.</p>
      ) : (
        <Markdown>{node.summary}</Markdown>
      )}
    </div>
  );
}

/** What the gate submits: its document at its commit, research's documents, or the summary. */
function GateDoc(
  props: Pick<PlanGateViewProps, "api" | "projectId" | "node" | "gate">,
): ReactElement {
  const { node, gate } = props;
  const path = gate.docPath;
  if (path !== undefined) {
    return (
      <PlanDocReader
        key={`${path}@${gate.sha ?? ""}`}
        api={props.api}
        projectId={props.projectId}
        path={path}
        sha={gate.sha}
        approvedSha={node.docs.find((d) => d.path === path)?.approvedSha}
      />
    );
  }
  if (researchAwaits(node, gate)) {
    return <PlanGateDocs api={props.api} projectId={props.projectId} node={node} />;
  }
  return <Summary node={node} />;
}

export default function PlanGateView(props: PlanGateViewProps): ReactElement {
  const { node, gate } = props;
  const panel = usePaneWidth(GATE_PANEL_WIDTH);
  const state = usePlanNode(props.api, node.id, node.version);
  const detail = state.status === "ready" ? state.detail : null;
  const extras = props.extras?.(node, gateItemIds(node, detail?.cards ?? []));
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
      <div className="plan-gate-steps">
        <PlanStepper node={node} />
      </div>
      <div className="plan-gate-body">
        <div className="plan-gate-doc">
          <GateDoc {...props} />
          {gate.prNumber === undefined ? null : (
            <p className="plan-dim">{`In pull request #${gate.prNumber}`}</p>
          )}
        </div>
        <PaneEdge label="Gate panel width" width={panel} />
        <PlanGatePanel
          style={panel.style}
          key={`${node.id}-${gate.round}`}
          api={props.api}
          node={node}
          gate={gate}
          canRule={props.canRule}
          who={props.who}
          onRuled={props.onRuled}
          detail={detail}
          extras={extras}
        />
      </div>
    </div>
  );
}

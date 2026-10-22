// The Plan tab (0.21.0, #43, canvas D1 and D2): the tree on the left; on
// the right the node picked or a document read in place. A gate for the
// owner's ruling takes the whole tab, with "‹ Plan" back to the tree. Live:
// `plan_change` pushes reach the tree while the tab watches the project.
// Needs you opens a node straight onto its gate.

import { useState } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { Bot } from "../../protocol/entities";
import type { PlanDoc, PlanNode, PlanTree } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import PlanCreateDialog from "./PlanCreateDialog";
import type { CreateKind } from "./PlanCreateDialog";
import PlanDocReader from "./PlanDocReader";
import PlanGateView from "./PlanGateView";
import type { GateExtras } from "./PlanGateView";
import PlanNodeDetail from "./PlanNodeDetail";
import type { TreeFilter } from "./planText";
import { actorName, ancestorsOf, fileName, treeRows } from "./planText";
import PlanTreePane from "./PlanTreePane";
import PaneEdge, { TREE_WIDTH } from "./PaneEdge";
import { usePaneWidth } from "../../hooks/usePaneWidth";

/** What the right side shows. */
type PlanPane =
  | { readonly kind: "node" }
  | { readonly kind: "gate" }
  | { readonly kind: "doc"; readonly doc: PlanDoc };

export interface PlanViewProps {
  readonly api: BoardApi;
  readonly projectId: string;
  readonly tree: PlanTree;
  readonly bots: readonly Bot[];
  readonly leadName?: string;
  /** The owner's: rules gates, skips research. */
  readonly canRule: boolean;
  /** Starts goals and initiatives (the control grant). */
  readonly canCreate: boolean;
  /** Takes a node an edit answered with. */
  readonly onChanged: (node: PlanNode) => void;
  /** The node a link opens, on its gate with `gate`. */
  readonly openNode?: string;
  readonly openGate?: boolean;
  readonly onOpenItem?: (id: string) => void;
  readonly onOpenBoard?: (boardId: string) => void;
  /** More on a gate beside the ruling: its push requests (0.24.0). */
  readonly gateExtras?: GateExtras;
  /** Stories: open with a filter, or a dialog showing. */
  readonly initial?: {
    readonly filter?: TreeFilter;
    readonly create?: CreateKind;
  };
}

/** The node a link asks for, until the owner picks another. */
function useOpened(props: PlanViewProps): {
  readonly selected: string | null;
  readonly pane: PlanPane;
  readonly open: (id: string, pane?: PlanPane) => void;
  readonly setPane: (pane: PlanPane) => void;
} {
  const first = treeRows(props.tree.nodes)[0]?.node.id ?? null;
  const asked = props.openNode ?? null;
  const askedPane: PlanPane = props.openGate === true ? { kind: "gate" } : { kind: "node" };
  const [selected, setSelected] = useState<string | null>(asked);
  const [pane, setPane] = useState<PlanPane>(askedPane);
  const [last, setLast] = useState({ asked, gate: props.openGate });
  if (asked !== last.asked || props.openGate !== last.gate) {
    setLast({ asked, gate: props.openGate });
    if (asked !== null) {
      setSelected(asked);
      setPane(askedPane);
    }
  }
  return {
    selected: selected ?? first,
    pane,
    open: (id, next = { kind: "node" }) => {
      setSelected(id);
      setPane(next);
    },
    setPane,
  };
}

function Right(props: PlanViewProps & ReturnType<typeof useOpened>): ReactElement {
  const { tree, pane } = props;
  const node = tree.nodes.find((n) => n.id === props.selected);
  const who = (actor: string): string =>
    actorName(actor, (id) => props.bots.find((b) => b.id === id)?.name);
  if (pane.kind === "doc") {
    return (
      <main className="plan-main" aria-label={fileName(pane.doc.path)}>
        <button type="button" className="plan-link" onClick={() => props.setPane({ kind: "node" })}>
          {node === undefined ? "‹ Back" : `‹ ${node.title}`}
        </button>
        <PlanDocReader
          key={pane.doc.path}
          api={props.api}
          projectId={props.projectId}
          path={pane.doc.path}
          approvedSha={pane.doc.changedSinceApproval ? pane.doc.approvedSha : undefined}
        />
      </main>
    );
  }
  if (node === undefined) {
    return (
      <main className="plan-main plan-main-empty">
        <p className="plan-empty">Pick a goal, initiative, epic or feature to see it here.</p>
      </main>
    );
  }
  const ancestors = ancestorsOf(tree.nodes, node);
  if (pane.kind === "gate" && node.gate !== undefined) {
    return (
      <PlanGateView
        api={props.api}
        projectId={props.projectId}
        node={node}
        gate={node.gate}
        ancestors={ancestors}
        canRule={props.canRule}
        who={who}
        onBack={() => props.setPane({ kind: "node" })}
        onRuled={(next) => {
          props.onChanged(next);
          props.setPane({ kind: "node" });
        }}
        extras={props.gateExtras}
      />
    );
  }
  return (
    <PlanNodeDetail
      api={props.api}
      node={node}
      ancestors={ancestors}
      who={who}
      canRule={props.canRule}
      canEdit={props.canCreate}
      bots={props.bots}
      leadName={props.leadName}
      onOpenNode={(id) => props.open(id)}
      onOpenGate={() => props.setPane({ kind: "gate" })}
      onOpenDoc={(doc) => props.setPane({ kind: "doc", doc })}
      onOpenItem={props.onOpenItem}
      onOpenBoard={props.onOpenBoard}
      onChanged={props.onChanged}
    />
  );
}

export default function PlanView(props: PlanViewProps): ReactElement {
  const opened = useOpened(props);
  const [filter, setFilter] = useState<TreeFilter>(props.initial?.filter ?? "all");
  const [creating, setCreating] = useState<CreateKind | null>(props.initial?.create ?? null);
  const selectedNode = props.tree.nodes.find((n) => n.id === opened.selected);
  const tree = usePaneWidth(TREE_WIDTH);
  return (
    <div className="plan-view">
      {opened.pane.kind === "gate" && selectedNode?.gate !== undefined ? null : (
        <>
          <PlanTreePane
            style={tree.style}
            nodes={props.tree.nodes}
            guidelines={props.tree.guidelines}
            selected={opened.selected}
            filter={filter}
            onFilter={setFilter}
            onSelect={(id) => opened.open(id)}
            onOpenDoc={(doc) => opened.setPane({ kind: "doc", doc })}
            onCreate={props.canCreate ? setCreating : undefined}
          />
          <PaneEdge label="Plan tree width" width={tree} />
        </>
      )}
      <Right {...props} {...opened} />
      {creating === null ? null : (
        <PlanCreateDialog
          api={props.api}
          projectId={props.projectId}
          kind={creating}
          nodes={props.tree.nodes}
          bots={props.bots}
          leadName={props.leadName}
          goalId={
            selectedNode === undefined
              ? undefined
              : (ancestorsOf(props.tree.nodes, selectedNode)[0] ?? selectedNode).id
          }
          onCreated={(node) => {
            setCreating(null);
            props.onChanged(node);
            opened.open(node.id);
          }}
          onCancel={() => setCreating(null)}
        />
      )}
    </div>
  );
}

// The plan's tree (0.21.0, canvas D1): goals, initiatives, epics and
// features, each with its stage chip; All, Needs you and Done; + Goal and
// + Initiative for the owner; the project's guidelines at the foot.

import type { CSSProperties, ReactElement } from "react";
import type { PlanDoc, PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import type { TreeFilter } from "./planText";
import {
  fileName,
  filteredRows,
  isFinished,
  kindWords,
  nodeMeta,
  questionsMeta,
  stageChip,
  stageWord,
} from "./planText";
import type { CreateKind } from "./PlanCreateDialog";

interface PlanTreePaneProps {
  readonly nodes: readonly PlanNode[];
  readonly guidelines: readonly PlanDoc[];
  readonly selected: string | null;
  readonly filter: TreeFilter;
  readonly onFilter: (filter: TreeFilter) => void;
  readonly onSelect: (id: string) => void;
  readonly onOpenDoc: (doc: PlanDoc) => void;
  /** + Goal and + Initiative; absent without the control grant. */
  readonly onCreate?: (kind: CreateKind) => void;
  /** Its width, which the owner drags (0.22.0). */
  readonly style?: CSSProperties;
}

function FilterButton(props: {
  readonly value: TreeFilter;
  readonly label: string;
  readonly count?: number;
  readonly filter: TreeFilter;
  readonly onFilter: (filter: TreeFilter) => void;
}): ReactElement {
  const on = props.filter === props.value;
  return (
    <button
      type="button"
      className={`plan-filter${on ? " plan-filter-on" : ""}`}
      aria-pressed={on}
      onClick={() => props.onFilter(props.value)}
    >
      {props.label}
      {props.count === undefined ? null : (
        <>
          {" "}
          <span className="plan-filter-count">{props.count}</span>
        </>
      )}
    </button>
  );
}

/** "Epic: Enemy attack telegraphing, Design", "…, HLD waiting for you": a row's name. */
function rowName(node: PlanNode): string {
  const stage =
    node.gate === undefined
      ? stageWord(node.stage)
      : `${stageWord(node.gate.stage)} waiting for you`;
  const meta = nodeMeta(node);
  const asks = questionsMeta(node);
  return `${kindWords(node.kind).word}: ${node.title}, ${stage}${meta === "" ? "" : `, ${meta}`}${asks === "" ? "" : `, ${asks}`}`;
}

function Row(props: {
  readonly node: PlanNode;
  readonly depth: number;
  readonly selected: boolean;
  readonly onSelect: (id: string) => void;
}): ReactElement {
  const { node } = props;
  const kind = kindWords(node.kind);
  const chip = stageChip(node);
  const meta = nodeMeta(node);
  return (
    <li>
      <button
        type="button"
        className={`plan-row plan-depth-${Math.min(props.depth, 3)}${props.selected ? " plan-row-on" : ""}${isFinished(node) ? " plan-row-finished" : ""}`}
        aria-current={props.selected ? "true" : undefined}
        aria-label={rowName(node)}
        onClick={() => props.onSelect(node.id)}
      >
        <span className={`plan-glyph plan-glyph-${kind.word.toLowerCase()}`} aria-hidden="true">
          {kind.glyph}
        </span>
        <span className="plan-row-title">{node.title}</span>
        {meta === "" ? null : <span className="plan-row-meta">{meta}</span>}
        {node.questionsOpen > 0 ? (
          <span
            className={`plan-row-questions${node.questionsBlocking > 0 ? " plan-row-questions-blocking" : ""}`}
            aria-hidden="true"
          >
            ?{node.questionsOpen}
          </span>
        ) : null}
        <span className={`plan-chip plan-chip-${chip.tone}`}>{chip.text}</span>
      </button>
    </li>
  );
}

export default function PlanTreePane(props: PlanTreePaneProps): ReactElement {
  const rows = filteredRows(props.nodes, props.filter);
  const waiting = props.nodes.filter((n) => n.gate !== undefined).length;
  const create = props.onCreate;
  return (
    <aside className="plan-tree" aria-label="Plan tree" style={props.style}>
      <div className="plan-tree-head">
        <h2>Plan</h2>
        {create === undefined ? null : (
          <>
            <button type="button" className="btn btn-small" onClick={() => create("goal")}>
              + Goal
            </button>
            <button type="button" className="btn btn-small" onClick={() => create("initiative")}>
              + Initiative
            </button>
          </>
        )}
      </div>
      <div className="plan-filters" role="group" aria-label="Show">
        <FilterButton value="all" label="All" filter={props.filter} onFilter={props.onFilter} />
        <FilterButton
          value="you"
          label="Needs you"
          count={waiting}
          filter={props.filter}
          onFilter={props.onFilter}
        />
        <FilterButton value="done" label="Done" filter={props.filter} onFilter={props.onFilter} />
      </div>
      {rows.length === 0 ? (
        <p className="plan-empty">
          {props.nodes.length === 0
            ? "No goals yet. Start with + Goal: what the project is for."
            : props.filter === "you"
              ? "Nothing waits for you."
              : "Nothing is done yet."}
        </p>
      ) : (
        <ul className="plan-rows" aria-label="Goals">
          {rows.map((row) => (
            <Row
              key={row.node.id}
              node={row.node}
              depth={row.depth}
              selected={row.node.id === props.selected}
              onSelect={props.onSelect}
            />
          ))}
        </ul>
      )}
      {props.guidelines.length === 0 ? null : (
        <section className="plan-guidelines" aria-label="Guidelines">
          <h3>Guidelines</h3>
          <p className="plan-dim">Every bot reads them before it works.</p>
          {props.guidelines.map((doc) => (
            <button
              key={doc.path}
              type="button"
              className="plan-link mono"
              onClick={() => props.onOpenDoc(doc)}
            >
              {fileName(doc.path)}
            </button>
          ))}
        </section>
      )}
    </aside>
  );
}

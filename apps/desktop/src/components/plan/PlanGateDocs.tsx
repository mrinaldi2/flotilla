// The documents of research waiting for the owner (0.24.0): the gate has no
// document of its own, so the node's research documents stand in, one per
// spike, read in place. A small list switches between them.

import { useState } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { PlanDocKind } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import PlanDocReader from "./PlanDocReader";
import { fileName } from "./planText";

export default function PlanGateDocs(props: {
  readonly api: BoardApi;
  readonly projectId: string;
  readonly node: PlanNode;
}): ReactElement {
  const docs = props.node.docs.filter((d) => d.kind === PlanDocKind.RESEARCH);
  const [picked, setPicked] = useState<string | null>(null);
  const doc = docs.find((d) => d.path === picked) ?? docs[0];
  if (doc === undefined) {
    return <p className="plan-empty">No research documents yet.</p>;
  }
  return (
    <>
      {docs.length < 2 ? null : (
        <ul className="plan-gate-doclist" aria-label="Research documents">
          {docs.map((d) => (
            <li key={d.path}>
              <button
                type="button"
                className="btn btn-small"
                aria-pressed={d.path === doc.path}
                onClick={() => setPicked(d.path)}
              >
                {fileName(d.path)}
                {d.itemId === undefined ? null : (
                  <>
                    {" "}
                    <span className="plan-dim">{`· ${d.itemId}`}</span>
                  </>
                )}
              </button>
            </li>
          ))}
        </ul>
      )}
      <PlanDocReader key={doc.path} api={props.api} projectId={props.projectId} path={doc.path} />
    </>
  );
}

// One node's detail (0.21.0): its history and its cards (`plan_node_get`),
// read again whenever the tree carries it to a new version.

import { useEffect, useState } from "react";
import type { BoardApi } from "../../protocol/board";
import { boardCall } from "../../protocol/board";
import type { PlanNodeDetail } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { errText } from "../../util";

export type NodeDetailState =
  | { readonly status: "loading" }
  | { readonly status: "ready"; readonly detail: PlanNodeDetail }
  | { readonly status: "failed"; readonly message: string };

export function usePlanNode(api: BoardApi, id: string, version: bigint): NodeDetailState {
  const [state, setState] = useState<{ readonly id: string; readonly state: NodeDetailState }>({
    id,
    state: { status: "loading" },
  });
  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const detail = await boardCall(api, { case: "planNodeGet", value: { id } }, "planNode");
        if (live) {
          setState({ id, state: { status: "ready", detail } });
        }
      } catch (failure) {
        if (live) {
          setState({ id, state: { status: "failed", message: errText(failure) } });
        }
      }
    })();
    return () => {
      live = false;
    };
    // `version` is a trigger, not an input: a push that moves the node reads it again.
    // oxlint-disable-next-line react/exhaustive-effect-dependencies
  }, [api, id, version]);
  // Another node's answer means this one's is still on its way.
  return state.id === id ? state.state : { status: "loading" };
}

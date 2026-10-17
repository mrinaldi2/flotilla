// A project's plan, live (0.21.0): `plan_get` once, then each `plan_change`
// push replaces its node, so the tree, the stage chips and the gates follow
// what the lead and the owner do. Without `plan_v1` the daemon has no plan,
// and nothing is asked.

import { useCallback, useEffect, useState } from "react";
import type { BoardApi } from "../../protocol/board";
import { boardCall } from "../../protocol/board";
import type { PlanNode, PlanTree } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { PLAN_V1 } from "../../protocol/plan";
import { errText } from "../../util";

export type PlanStatus =
  | { readonly kind: "unsupported" }
  | { readonly kind: "loading" }
  | { readonly kind: "failed"; readonly message: string }
  | { readonly kind: "ready"; readonly tree: PlanTree };

export interface UsePlan {
  readonly status: PlanStatus;
  /** Reads the tree again. */
  readonly refresh: () => void;
  /** Takes a node an edit answered with, unless a push already carried it further. */
  readonly apply: (node: PlanNode) => void;
}

/** The tree with `node` in it: a newer version replaces the old, a new node joins. */
export function withNode(tree: PlanTree, node: PlanNode): PlanTree {
  const at = tree.nodes.findIndex((n) => n.id === node.id);
  if (at < 0) {
    return { ...tree, nodes: [...tree.nodes, node] };
  }
  const old = tree.nodes[at];
  if (old !== undefined && old.version > node.version) {
    return tree;
  }
  return { ...tree, nodes: tree.nodes.map((n, i) => (i === at ? node : n)) };
}

/** Whether the project's plan is on: the daemon serves it and the owner turned it on. */
export function planOn(status: PlanStatus): boolean {
  return status.kind === "ready" && status.tree.enabled;
}

/** The plan's nodes while it is on; nothing otherwise. */
export function nodesWhenOn(status: PlanStatus): readonly PlanNode[] | undefined {
  return status.kind === "ready" && status.tree.enabled ? status.tree.nodes : undefined;
}

export function usePlan(api: BoardApi, projectId: string, live: boolean): UsePlan {
  const supported = api.capabilities.includes(PLAN_V1);
  const [status, setStatus] = useState<PlanStatus>({ kind: "loading" });
  const [asked, setAsked] = useState(0);

  useEffect(() => {
    if (!supported || !live) {
      return undefined;
    }
    let current = true;
    const off = api.onPlanChange((change) => {
      const node = change.node;
      if (change.projectId === projectId && node !== undefined) {
        setStatus((s) =>
          s.kind === "ready" ? { kind: "ready", tree: withNode(s.tree, node) } : s,
        );
      }
    });
    void (async () => {
      try {
        const tree = await boardCall(api, { case: "planGet", value: { projectId } }, "plan");
        if (current) {
          setStatus({ kind: "ready", tree });
        }
      } catch (failure) {
        if (current) {
          setStatus({ kind: "failed", message: errText(failure) });
        }
      }
    })();
    return () => {
      current = false;
      off();
    };
    // `asked` is a trigger, not an input: `refresh` reads the tree again.
    // oxlint-disable-next-line react/exhaustive-effect-dependencies
  }, [api, projectId, live, supported, asked]);

  const refresh = useCallback((): void => {
    setAsked((n) => n + 1);
  }, []);
  const apply = useCallback((node: PlanNode): void => {
    setStatus((s) => (s.kind === "ready" ? { kind: "ready", tree: withNode(s.tree, node) } : s));
  }, []);
  return { status: supported ? status : { kind: "unsupported" }, refresh, apply };
}

/**
 * Watches the project while the Plan tab shows, so `plan_change` pushes
 * arrive; the Board tab holds its own watch while it shows.
 */
export function usePlanWatch(api: BoardApi, projectId: string, live: boolean): void {
  useEffect(() => {
    if (!live) {
      return undefined;
    }
    const value = { projectId };
    api.board({ case: "boardWatch", value }).catch(() => {
      // A project without a board yet has nothing to watch; the tree still reads.
    });
    return () => {
      api.board({ case: "boardUnwatch", value }).catch(() => {
        // The watch ends with the connection anyway.
      });
    };
  }, [api, projectId, live]);
}

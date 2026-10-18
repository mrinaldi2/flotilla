// The plan on an initiative's board (0.21.0): "Belongs to", the node a new
// card joins (tasks and bugs a feature in Build; spikes and design tickets
// an epic or initiative in research); the node chip on each card; and the
// filter by epic or feature, which keeps the cards of a node and of every
// node under it.

import { ItemType } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { PlanKind, PlanStage } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { isFinished, kindWords, stageWord } from "./planText";

/** The nodes a card on `boardId` can belong to: its initiative's live subtree. */
export function nodeOptions(
  nodes: readonly PlanNode[],
  boardId: string | undefined,
): readonly PlanNode[] {
  if (boardId === undefined) {
    return [];
  }
  return nodes.filter(
    (n) =>
      n.boardId === boardId &&
      n.kind !== PlanKind.GOAL &&
      n.stage !== PlanStage.PROPOSED &&
      !isFinished(n),
  );
}

/** "Feature · Hitstop on heavy hits", "Epic · Hit feedback (research)". */
export function optionLabel(node: PlanNode): string {
  const stage =
    node.stage === PlanStage.RESEARCH ? ` (${stageWord(node.stage).toLowerCase()})` : "";
  return `${kindWords(node.kind).word} · ${node.title}${stage}`;
}

/** Where a card of `type` most likely goes; the first option otherwise. */
export function preferredNode(options: readonly PlanNode[], type: ItemType): string | undefined {
  const research = type === ItemType.SPIKE || type === ItemType.DESIGN;
  const fits = options.find((n) =>
    research
      ? n.stage === PlanStage.RESEARCH
      : n.kind === PlanKind.FEATURE && n.stage === PlanStage.BUILD,
  );
  return (fits ?? options[0])?.id;
}

/** The epics and features the board's filter offers: live ones on `boardId`. */
export function filterNodes(
  nodes: readonly PlanNode[],
  boardId: string | undefined,
): readonly PlanNode[] {
  return nodeOptions(nodes, boardId).filter(
    (n) => n.kind === PlanKind.EPIC || n.kind === PlanKind.FEATURE,
  );
}

/** For each node, itself and every node under it, by id. */
export function subtrees(nodes: readonly PlanNode[]): ReadonlyMap<string, ReadonlySet<string>> {
  const children = new Map<string, string[]>();
  for (const node of nodes) {
    if (node.parentId !== undefined) {
      children.set(node.parentId, [...(children.get(node.parentId) ?? []), node.id]);
    }
  }
  const scope = new Map<string, ReadonlySet<string>>();
  for (const node of nodes) {
    const under = new Set<string>();
    const queue = [node.id];
    for (let id = queue.pop(); id !== undefined; id = queue.pop()) {
      if (!under.has(id)) {
        under.add(id);
        queue.push(...(children.get(id) ?? []));
      }
    }
    scope.set(node.id, under);
  }
  return scope;
}

// "Belongs to" (0.21.0): the plan node a new card joins on an initiative's
// board. Tasks and bugs belong to a feature in Build; spikes and design
// tickets to an epic or initiative in research.

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

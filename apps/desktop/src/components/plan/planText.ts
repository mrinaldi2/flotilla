// The plan's words (0.21.0, #43): each level and stage in words and a glyph,
// so nothing depends on colour; the stages a kind passes, gates marked; and
// the tree in reading order, with the Needs you and Done filters.

import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import {
  PlanDocKind,
  PlanKind,
  PlanResearchState,
  PlanStage,
} from "../../protocol/gen/flotilla/board/v1/plan_pb";

interface KindWords {
  readonly glyph: string;
  readonly word: string;
}

const KIND: Readonly<Record<PlanKind, KindWords>> = {
  [PlanKind.UNSPECIFIED]: { glyph: "•", word: "Node" },
  [PlanKind.GOAL]: { glyph: "G", word: "Goal" },
  [PlanKind.INITIATIVE]: { glyph: "I", word: "Initiative" },
  [PlanKind.EPIC]: { glyph: "E", word: "Epic" },
  [PlanKind.FEATURE]: { glyph: "F", word: "Feature" },
};

export function kindWords(kind: PlanKind): KindWords {
  return KIND[kind] ?? KIND[PlanKind.UNSPECIFIED];
}

const STAGE: Readonly<Record<PlanStage, string>> = {
  [PlanStage.UNSPECIFIED]: "Unknown",
  [PlanStage.DEFINE]: "Define",
  [PlanStage.ACTIVE]: "Active",
  [PlanStage.MET]: "Met",
  [PlanStage.DROPPED]: "Dropped",
  [PlanStage.PROPOSED]: "Proposed",
  [PlanStage.RESEARCH]: "Research",
  [PlanStage.PRD]: "PRD",
  [PlanStage.DESIGN]: "Design",
  [PlanStage.HLD]: "HLD",
  [PlanStage.LLD]: "LLD",
  [PlanStage.RUNNING]: "Running",
  [PlanStage.BUILD]: "Build",
  [PlanStage.RELEASED]: "Released",
  [PlanStage.DONE]: "Done",
};

export function stageWord(stage: PlanStage): string {
  return STAGE[stage] ?? STAGE[PlanStage.UNSPECIFIED];
}

const GATES: ReadonlySet<PlanStage> = new Set([
  PlanStage.PRD,
  PlanStage.DESIGN,
  PlanStage.HLD,
  PlanStage.LLD,
]);

/** A stage the owner approves a document at. */
export function isGateStage(stage: PlanStage): boolean {
  return GATES.has(stage);
}

const FINISHED: ReadonlySet<PlanStage> = new Set([
  PlanStage.MET,
  PlanStage.DROPPED,
  PlanStage.RELEASED,
  PlanStage.DONE,
]);

/** Done with: met, released, done or dropped. */
export function isFinished(node: PlanNode): boolean {
  return FINISHED.has(node.stage);
}

/** The stages a kind passes, in order (`plan.proto`); Dropped is off the path. */
export function stagesOf(kind: PlanKind): readonly PlanStage[] {
  switch (kind) {
    case PlanKind.GOAL:
      return [PlanStage.DEFINE, PlanStage.ACTIVE, PlanStage.MET];
    case PlanKind.INITIATIVE:
      return [PlanStage.RESEARCH, PlanStage.PRD, PlanStage.RUNNING, PlanStage.DONE];
    case PlanKind.EPIC:
      return [
        PlanStage.RESEARCH,
        PlanStage.DESIGN,
        PlanStage.HLD,
        PlanStage.RUNNING,
        PlanStage.DONE,
      ];
    case PlanKind.FEATURE:
      return [PlanStage.LLD, PlanStage.BUILD, PlanStage.RELEASED];
    default:
      return [];
  }
}

/** How a stage chip reads: waiting for the owner, a gate, research, or plain. */
type StageTone = "you" | "gate" | "research" | "plain";

export interface StageChip {
  readonly text: string;
  readonly tone: StageTone;
}

/** "HLD · you" while a gate waits for the owner; else the stage in words. */
export function stageChip(node: PlanNode): StageChip {
  if (node.gate !== undefined) {
    return { text: `${stageWord(node.gate.stage)} · you`, tone: "you" };
  }
  if (isGateStage(node.stage)) {
    return { text: stageWord(node.stage), tone: "gate" };
  }
  if (node.stage === PlanStage.RESEARCH) {
    return { text: stageWord(node.stage), tone: "research" };
  }
  return { text: stageWord(node.stage), tone: "plain" };
}

/** "3/5", "24 cards", "1 spike": the short line beside a tree row. */
export function nodeMeta(node: PlanNode): string {
  if (node.kind === PlanKind.FEATURE && node.cardsTotal > 0) {
    return `${node.cardsDone}/${node.cardsTotal}`;
  }
  const spikes = node.research?.state === PlanResearchState.OPEN ? node.research.spikes.length : 0;
  if (node.stage === PlanStage.RESEARCH && spikes > 0) {
    return `${spikes} ${spikes === 1 ? "spike" : "spikes"}`;
  }
  if (node.kind === PlanKind.INITIATIVE && node.cardsTotal > 0) {
    return `${node.cardsTotal} ${node.cardsTotal === 1 ? "card" : "cards"}`;
  }
  return "";
}

const DOC: Readonly<Record<PlanDocKind, string>> = {
  [PlanDocKind.UNSPECIFIED]: "Document",
  [PlanDocKind.GOAL]: "Goal",
  [PlanDocKind.RESEARCH]: "Research",
  [PlanDocKind.PRD]: "PRD",
  [PlanDocKind.DESIGN]: "Design",
  [PlanDocKind.HLD]: "HLD",
  [PlanDocKind.LLD]: "LLD",
  [PlanDocKind.GUIDELINE]: "Guideline",
};

export function docWord(kind: PlanDocKind): string {
  return DOC[kind] ?? DOC[PlanDocKind.UNSPECIFIED];
}

/** The file's own name: `docs/x/prd.md` → `prd.md`. */
export function fileName(path: string): string {
  return path.slice(path.lastIndexOf("/") + 1);
}

/** One row of the tree: the node and how deep it sits. */
export interface TreeRow {
  readonly node: PlanNode;
  readonly depth: number;
}

function byRank(a: PlanNode, b: PlanNode): number {
  if (a.rank === b.rank) {
    return a.id.localeCompare(b.id);
  }
  return a.rank < b.rank ? -1 : 1;
}

/**
 * The tree in reading order: each node under its parent, siblings by rank.
 * A node whose parent isn't listed sits at the top, so nothing is lost.
 */
export function treeRows(nodes: readonly PlanNode[]): readonly TreeRow[] {
  const ids = new Set(nodes.map((n) => n.id));
  const children = new Map<string, PlanNode[]>();
  const roots: PlanNode[] = [];
  for (const node of nodes) {
    const parent = node.parentId;
    if (parent === undefined || !ids.has(parent)) {
      roots.push(node);
    } else {
      children.set(parent, [...(children.get(parent) ?? []), node]);
    }
  }
  const rows: TreeRow[] = [];
  const walk = (list: readonly PlanNode[], depth: number): void => {
    // `sort` on a fresh copy, not `toSorted`: the build targets ES2021.
    // oxlint-disable-next-line unicorn/no-array-sort
    for (const node of [...list].sort(byRank)) {
      rows.push({ node, depth });
      walk(children.get(node.id) ?? [], depth + 1);
    }
  };
  walk(roots, 0);
  return rows;
}

export type TreeFilter = "all" | "you" | "done";

/** The node and every ancestor of it, by id. */
function withAncestors(nodes: readonly PlanNode[], keep: (n: PlanNode) => boolean): Set<string> {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const kept = new Set<string>();
  for (const node of nodes.filter(keep)) {
    let at: PlanNode | undefined = node;
    while (at !== undefined && !kept.has(at.id)) {
      kept.add(at.id);
      at = at.parentId === undefined ? undefined : byId.get(at.parentId);
    }
  }
  return kept;
}

/** The rows a filter shows: what waits for the owner, or what is finished, with their parents. */
export function filteredRows(nodes: readonly PlanNode[], filter: TreeFilter): readonly TreeRow[] {
  const rows = treeRows(nodes);
  if (filter === "all") {
    return rows;
  }
  const kept = withAncestors(nodes, filter === "you" ? (n) => n.gate !== undefined : isFinished);
  return rows.filter((row) => kept.has(row.node.id));
}

/** The node's ancestors, the goal first: the breadcrumb. */
export function ancestorsOf(nodes: readonly PlanNode[], node: PlanNode): readonly PlanNode[] {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const line: PlanNode[] = [];
  let parent = node.parentId === undefined ? undefined : byId.get(node.parentId);
  while (parent !== undefined && !line.includes(parent)) {
    line.unshift(parent);
    parent = parent.parentId === undefined ? undefined : byId.get(parent.parentId);
  }
  return line;
}

/** "bot:b-lead" in words: the bot's name, or "you" for the owner. */
export function actorName(actor: string, botName: (id: string) => string | undefined): string {
  if (actor === "user" || actor === "owner" || actor.startsWith("device:")) {
    return "You";
  }
  if (actor === "daemon") {
    return "Flotilla";
  }
  const id = actor.startsWith("bot:") ? actor.slice(4) : actor;
  return botName(id) ?? "a bot";
}

import { create } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import { ItemType } from "../../protocol/gen/flotilla/board/v1/board_pb";
import {
  PlanEventSchema,
  PlanKind,
  PlanNodeSchema,
  PlanStage,
} from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { hudEpic, PLAN_BOARD, planNodes, telegraphEpic } from "../../test/planFixtures";
import { nodeOptions, optionLabel, preferredNode } from "./belongsTo";
import { historyLine } from "./planHistory";
import {
  actorName,
  ancestorsOf,
  filteredRows,
  kindWords,
  nodeMeta,
  stageChip,
  stagesOf,
  stageWord,
  treeRows,
} from "./planText";

const bots = (id: string): string | undefined => (id === "lead" ? "Game Lead" : undefined);
const who = (actor: string): string => (actor === "user" ? "You" : "Game Lead");
const event = (kind: string, note?: string) =>
  create(PlanEventSchema, { actor: "user", kind, stage: PlanStage.HLD, note });

const titles = (rows: readonly { readonly node: { readonly title: string } }[]): string[] =>
  rows.map((r) => r.node.title);

describe("the plan's tree", () => {
  it("puts each node under its parent, siblings by rank", () => {
    const rows = treeRows(planNodes());
    expect(rows.map((r) => [r.node.id, r.depth])).toEqual([
      ["PL-1", 0],
      ["PL-2", 1],
      ["PL-4", 2],
      ["PL-5", 2],
      ["PL-6", 3],
      ["PL-7", 3],
      ["PL-8", 2],
      ["PL-3", 1],
      ["PL-9", 1],
    ]);
  });

  it("keeps a node whose parent isn't listed, at the top", () => {
    const orphan = create(PlanNodeSchema, { id: "PL-99", parentId: "PL-gone", title: "Orphan" });
    expect(treeRows([orphan])).toEqual([{ node: orphan, depth: 0 }]);
  });

  it("filters to what waits for the owner, or what is done, with their parents", () => {
    expect(titles(filteredRows(planNodes(), "you"))).toEqual([
      "Improve combat UX",
      "Combat readability",
      "Combat HUD clarity",
    ]);
    expect(filteredRows(planNodes(), "done")).toEqual([]);
    expect(filteredRows(planNodes(), "all")).toHaveLength(9);
  });

  it("names a node's ancestors, the goal first", () => {
    const nodes = planNodes();
    const feature = nodes.find((n) => n.id === "PL-6");
    expect(feature && titles(ancestorsOf(nodes, feature).map((node) => ({ node })))).toEqual([
      "Improve combat UX",
      "Combat readability",
      "Hit feedback",
    ]);
  });
});

describe("the plan's words", () => {
  it("chips a gate waiting for the owner, a gate stage, research and the rest", () => {
    expect(stageChip(hudEpic())).toEqual({ text: "HLD · you", tone: "you" });
    expect(stageChip(telegraphEpic())).toEqual({ text: "Design", tone: "gate" });
    const research = create(PlanNodeSchema, { stage: PlanStage.RESEARCH });
    expect(stageChip(research)).toEqual({ text: "Research", tone: "research" });
    expect(stageChip(create(PlanNodeSchema, { stage: PlanStage.RUNNING })).tone).toBe("plain");
  });

  it("counts a feature's cards, open research and an initiative's cards", () => {
    const [, initiative, , , feature, , , , onboarding] = planNodes();
    expect(feature && nodeMeta(feature)).toBe("3/5");
    expect(initiative && nodeMeta(initiative)).toBe("24 cards");
    expect(onboarding && nodeMeta(onboarding)).toBe("1 spike");
    expect(nodeMeta(hudEpic())).toBe("");
  });

  it("lists each kind's stages", () => {
    expect(stagesOf(PlanKind.EPIC).map(stageWord)).toEqual([
      "Research",
      "Design",
      "HLD",
      "Running",
      "Done",
    ]);
    expect(stagesOf(PlanKind.FEATURE).map(stageWord)).toEqual(["LLD", "Build", "Released"]);
    expect(stagesOf(PlanKind.UNSPECIFIED)).toEqual([]);
    expect(kindWords(PlanKind.GOAL)).toEqual({ glyph: "G", word: "Goal" });
  });

  it("names who did something", () => {
    expect(actorName("user", bots)).toBe("You");
    expect(actorName("device:ipad", bots)).toBe("You");
    expect(actorName("daemon", bots)).toBe("Flotilla");
    expect(actorName("bot:lead", bots)).toBe("Game Lead");
    expect(actorName("bot:gone", bots)).toBe("a bot");
  });

  it("words the history, and a kind it doesn't know", () => {
    expect(historyLine(event("sent_back", "Add the risks."), who)).toBe(
      "You sent the HLD back: Add the risks.",
    );
    expect(historyLine(event("approved"), who)).toBe("You approved the HLD");
    expect(historyLine(event("rescoped", "Narrower"), who)).toBe("You: Narrower");
    expect(historyLine(create(PlanEventSchema, { actor: "x", kind: "stage" }), who)).toBe(
      "Moved to document",
    );
  });
});

describe("a new card's node", () => {
  it("offers the live nodes of the initiative's board", () => {
    const options = nodeOptions(planNodes(), PLAN_BOARD);
    expect(options.map((n) => n.id)).toEqual(["PL-2", "PL-4", "PL-5", "PL-6", "PL-7", "PL-8"]);
    expect(nodeOptions(planNodes(), undefined)).toEqual([]);
    expect(nodeOptions(planNodes(), "p1/main")).toEqual([]);
  });

  it("picks a feature in Build for a task, research for a spike", () => {
    const options = nodeOptions(planNodes(), PLAN_BOARD);
    expect(preferredNode(options, ItemType.TASK)).toBe("PL-6");
    expect(preferredNode(options, ItemType.SPIKE)).toBe("PL-2");
    expect(preferredNode([], ItemType.TASK)).toBeUndefined();
  });

  it("labels an option by its kind, and research in brackets", () => {
    const research = create(PlanNodeSchema, {
      kind: PlanKind.EPIC,
      title: "Hit feedback",
      stage: PlanStage.RESEARCH,
    });
    expect(optionLabel(research)).toBe("Epic · Hit feedback (research)");
    expect(optionLabel(hudEpic())).toBe("Epic · Combat HUD clarity");
  });
});

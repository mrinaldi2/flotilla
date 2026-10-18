// What the Board tab holds while it shows (0.19.0): the owner's prefs, the
// board picked (the item's own board for a link, else the one asked for,
// remembered or default), the open card or the settings panel, and that
// board's columns and cards.

import { useCallback, useMemo, useState } from "react";
import type { Bot } from "../../protocol/entities";
import type { Board, BoardColumn, ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { filterNodes, subtrees } from "../plan/belongsTo";
import type { BotOption } from "./BoardFilterBar";
import type { BoardPrefs } from "./boardPrefs";
import { loadBoardPrefs, saveBoardPrefs } from "./boardPrefs";
import { boardOfItem, cardsOf, columnsOf, pickBoard } from "./boards";
import type { BoardModel } from "./boardSync";
import type { NodeScope } from "./filters";
import { matchesFilters } from "./filters";
import type { LabelGroupView } from "./labelGroups";
import { boardLabelGroups } from "./labelGroups";
import type { SettingsSection } from "./settings/BoardSettings";
import { useOpenedItem } from "./useOpenedItem";

export interface BoardView {
  readonly prefs: BoardPrefs;
  readonly updatePrefs: (next: BoardPrefs) => void;
  readonly board: Board | undefined;
  readonly columns: readonly BoardColumn[];
  /** The board's cards, in rank order. */
  readonly cards: readonly ItemCard[];
  /** Those the filters leave. */
  readonly shown: readonly ItemCard[];
  readonly botsById: ReadonlyMap<string, Bot>;
  readonly botOptions: readonly BotOption[];
  /** The board's label groups and those its cards use, for the filters. */
  readonly labelGroups: readonly LabelGroupView[];
  /** The groups whose labels the cards show: the board's own, or, from a
   * home that names no boards, every group the cards use. */
  readonly chipGroups: ReadonlySet<string>;
  /** The plan's nodes by id, for the cards' node chips (0.21.0). */
  readonly nodesById: ReadonlyMap<string, PlanNode>;
  /** Each node and those under it, for the node filter; none without a plan. */
  readonly scope: NodeScope | undefined;
  /** The board's epics and features, as filter chips. */
  readonly filterNodes: readonly PlanNode[];
  /** `bot:<id>` in words. */
  readonly who: (actor: string) => string;
  readonly opened: string | null;
  readonly openItem: (id: string | null) => void;
  readonly select: (boardId: string) => void;
  readonly settings: SettingsSection | null;
  readonly setSettings: (section: SettingsSection | null) => void;
}

function byRank(a: ItemCard, b: ItemCard): number {
  if (a.rank === b.rank) {
    return a.id.localeCompare(b.id);
  }
  return a.rank < b.rank ? -1 : 1;
}

/** The board a link asks for, until the owner picks another. */
function useAsked(openBoard: string | undefined): [string | undefined, () => void] {
  const [asked, setAsked] = useState(openBoard);
  const [last, setLast] = useState(openBoard);
  if (openBoard !== last) {
    setLast(openBoard);
    setAsked(openBoard);
  }
  return [asked, () => setAsked(undefined)];
}

function useBotOptions(bots: readonly Bot[], cards: readonly ItemCard[]): readonly BotOption[] {
  return useMemo((): readonly BotOption[] => {
    const options: BotOption[] = bots.map((bot) => ({ id: bot.id, bot }));
    for (const card of cards) {
      const id = card.assignee;
      if (id !== undefined && !options.some((option) => option.id === id)) {
        options.push({ id, bot: undefined });
      }
    }
    return options;
  }, [bots, cards]);
}

/** The plan's part of the board: node chips, the node filter's scope and its chips. */
function usePlanView(
  nodes: readonly PlanNode[] | undefined,
  boardId: string | undefined,
): Pick<BoardView, "nodesById" | "scope" | "filterNodes"> {
  return useMemo(
    () => ({
      nodesById: new Map((nodes ?? []).map((n) => [n.id, n])),
      scope: nodes === undefined ? undefined : subtrees(nodes),
      filterNodes: filterNodes(nodes ?? [], boardId),
    }),
    [nodes, boardId],
  );
}

export function useBoardView(opts: {
  readonly projectId: string;
  readonly model: BoardModel;
  readonly bots: readonly Bot[];
  readonly openItem?: string;
  readonly openBoard?: string;
  readonly settings?: SettingsSection;
  /** The project's plan nodes while planning is on (0.21.0). */
  readonly planNodes?: readonly PlanNode[];
}): BoardView {
  const { model, bots } = opts;
  const [prefs, setPrefs] = useState<BoardPrefs>(() => loadBoardPrefs(opts.projectId));
  const [opened, setOpened] = useOpenedItem(opts.openItem);
  const [asked, forgetAsked] = useAsked(opts.openBoard);
  const [settings, setSettings] = useState<SettingsSection | null>(opts.settings ?? null);
  const updatePrefs = useCallback(
    (next: BoardPrefs): void => {
      setPrefs(next);
      saveBoardPrefs(opts.projectId, next);
    },
    [opts.projectId],
  );
  const board = pickBoard(model, { asked, item: opened ?? undefined, remembered: prefs.board });
  const columns = useMemo(() => columnsOf(model, board), [model, board]);
  const cards = useMemo(() => {
    const sorted = cardsOf([...model.cards.values()], board);
    // oxlint-disable-next-line unicorn/no-array-sort
    sorted.sort(byRank);
    return sorted;
  }, [model.cards, board]);
  const botsById = useMemo(() => new Map(bots.map((bot) => [bot.id, bot])), [bots]);
  const botOptions = useBotOptions(bots, cards);
  const plan = usePlanView(opts.planNodes, board?.id);
  const labelGroups = useMemo(
    () =>
      boardLabelGroups(
        board,
        cards.map((card) => card.labels),
      ),
    [board, cards],
  );
  const chipGroups = useMemo(
    () =>
      new Set((board === undefined ? labelGroups : board.labelGroups).map((group) => group.name)),
    [board, labelGroups],
  );
  // The board shown is the one remembered, however it was reached.
  const remember = (boardId: string | undefined): void => {
    if (boardId !== undefined && boardId !== prefs.board) {
      forgetAsked();
      updatePrefs({ ...prefs, board: boardId });
    }
  };
  return {
    prefs,
    updatePrefs,
    board,
    columns,
    cards,
    shown: cards.filter((card) => matchesFilters(card, prefs.filters, plan.scope)),
    ...plan,
    botsById,
    botOptions,
    labelGroups,
    chipGroups,
    who: (actor) =>
      botsById.get(actor.startsWith("bot:") ? actor.slice(4) : actor)?.name ?? "the lead",
    opened,
    openItem: (id) => {
      setSettings(null);
      setOpened(id);
      // The item's own board, or the one a link asked for once the drawer closes.
      remember(id === null ? board?.id : boardOfItem(model, id));
    },
    select: (boardId) => {
      forgetAsked();
      setOpened(null);
      updatePrefs({ ...prefs, board: boardId });
    },
    settings,
    setSettings: (section) => {
      if (section !== null) {
        setOpened(null);
      }
      setSettings(section);
    },
  };
}

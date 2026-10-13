// What the Board tab holds while it shows (0.19.0): the owner's prefs, the
// board picked (the item's own board for a link, else the one asked for,
// remembered or default), the open card or the settings panel, and that
// board's columns and cards.

import { useCallback, useMemo, useState } from "react";
import type { Bot } from "../../protocol/entities";
import type { Board, BoardColumn, ItemCard } from "../../protocol/gen/hermes/board/v1/board_pb";
import type { BotOption } from "./BoardFilterBar";
import type { BoardPrefs } from "./boardPrefs";
import { loadBoardPrefs, saveBoardPrefs } from "./boardPrefs";
import { boardOfItem, cardsOf, columnsOf, pickBoard } from "./boards";
import type { BoardModel } from "./boardSync";
import { matchesFilters } from "./filters";
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

export function useBoardView(opts: {
  readonly projectId: string;
  readonly model: BoardModel;
  readonly bots: readonly Bot[];
  readonly openItem?: string;
  readonly openBoard?: string;
  readonly settings?: SettingsSection;
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
    shown: cards.filter((card) => matchesFilters(card, prefs.filters)),
    botsById,
    botOptions,
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

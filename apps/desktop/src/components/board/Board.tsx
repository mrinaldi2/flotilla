// One project's board, live (H-018 §3; 0.19.0): the board picked in the
// switcher (remembered per project; a link to an item opens its own board),
// its toolbar, the lead's latest change, its columns, and on the right the
// item drawer or the board's settings.

import { useCallback, useMemo, useState } from "react";
import type { ReactElement } from "react";
import type { AddToast } from "../../app/useToasts";
import type { DaemonApi } from "../../protocol/api";
import { boardCall } from "../../protocol/board";
import type { Bot, Project } from "../../protocol/entities";
import type { BoardColumn, ItemCard } from "../../protocol/gen/hermes/board/v1/board_pb";
import type { BoardSnapshot } from "../../protocol/gen/hermes/board/v1/requests_pb";
import { useNow } from "../control/useNow";
import { ItemOwnerActions } from "../ownerActions/OwnerActionList";
import BoardColumns from "./BoardColumns";
import BoardDrawer from "./BoardDrawer";
import BoardFilterBar from "./BoardFilterBar";
import type { BotOption } from "./BoardFilterBar";
import type { BoardPrefs } from "./boardPrefs";
import { loadBoardPrefs, saveBoardPrefs } from "./boardPrefs";
import { boardOfItem, cardsOf, columnsOf, pickBoard } from "./boards";
import type { BoardModel } from "./boardSync";
import BoardToolbar from "./BoardToolbar";
import DraftsTray from "./drafts/DraftsTray";
import NewItemDialog from "./drafts/NewItemDialog";
import { matchesFilters } from "./filters";
import LeadNotice from "./LeadNotice";
import { MoveDialog, MoveMenu, RefusalPopover } from "./MovePopovers";
import NewBoardSheet from "./NewBoardSheet";
import type { NewBoard } from "./NewBoardSheet";
import BoardSettings from "./settings/BoardSettings";
import type { SettingsSection } from "./settings/BoardSettings";
import { useBoardMoves } from "./useBoardMoves";
import { useOpenedItem } from "./useOpenedItem";

export interface BoardProps {
  readonly client: DaemonApi;
  readonly project: Project;
  readonly bots: readonly Bot[];
  readonly connected: boolean;
  readonly canControl: boolean;
  readonly addToast: AddToast;
  readonly openItem?: string;
  /** A board to show, from a link. */
  readonly openBoard?: string;
  readonly model: BoardModel;
  readonly refresh: () => void;
  readonly adopt: (snapshot: BoardSnapshot) => void;
  /** Stories: open with a panel or sheet showing. */
  readonly initial?: {
    readonly settings?: SettingsSection;
    readonly expanded?: string;
    readonly sheet?: "board" | "item";
    readonly switcher?: boolean;
    readonly drafts?: boolean;
  };
  /** "Now", for ages; stories pin it. */
  readonly now?: number;
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

export default function Board(props: BoardProps): ReactElement {
  const { client, project, model, bots, canControl, addToast, refresh, adopt } = props;
  const [prefs, setPrefs] = useState<BoardPrefs>(() => loadBoardPrefs(project.id));
  const [opened, setOpened] = useOpenedItem(props.openItem);
  const [asked, forgetAsked] = useAsked(props.openBoard);
  const [settings, setSettings] = useState<SettingsSection | null>(props.initial?.settings ?? null);
  const [sheet, setSheet] = useState(props.initial?.sheet ?? null);
  const [showArchived, setShowArchived] = useState(false);
  const clock = useNow();
  const now = props.now ?? clock;

  const updatePrefs = useCallback(
    (next: BoardPrefs): void => {
      setPrefs(next);
      saveBoardPrefs(project.id, next);
    },
    [project.id],
  );
  const board = pickBoard(model, { asked, item: opened ?? undefined, remembered: prefs.board });

  const columns = useMemo(() => columnsOf(model, board), [model, board]);
  const cards = useMemo(() => {
    const sorted = cardsOf([...model.cards.values()], board);
    // oxlint-disable-next-line unicorn/no-array-sort
    sorted.sort(byRank);
    return sorted;
  }, [model.cards, board]);
  const drafts = [...model.drafts.values()];
  const moves = useBoardMoves({ api: client, columns, refresh, addToast });
  const botsById = useMemo(() => new Map(bots.map((bot) => [bot.id, bot])), [bots]);
  const who = (actor: string): string =>
    botsById.get(actor.startsWith("bot:") ? actor.slice(4) : actor)?.name ?? "the lead";
  const botOptions = useMemo((): readonly BotOption[] => {
    const options: BotOption[] = bots.map((bot) => ({ id: bot.id, bot }));
    for (const card of cards) {
      const id = card.assignee;
      if (id !== undefined && !options.some((option) => option.id === id)) {
        options.push({ id, bot: undefined });
      }
    }
    return options;
  }, [bots, cards]);

  const canStructure = model.canRule && client.hasGrant("approve");
  const shown = cards.filter((card) => matchesFilters(card, prefs.filters));
  const select = (boardId: string): void => {
    forgetAsked();
    setOpened(null);
    updatePrefs({ ...prefs, board: boardId });
  };
  const openItem = (id: string | null): void => {
    setSettings(null);
    setOpened(id);
    // The board shown is the one remembered, however it was reached: the
    // item's own board, or the one a link asked for once the drawer closes.
    const on = id === null ? board?.id : boardOfItem(model, id);
    if (on !== undefined && on !== prefs.board) {
      forgetAsked();
      updatePrefs({ ...prefs, board: on });
    }
  };
  // The owner's call, on the board's home (H-099).
  const setLimit =
    (column: BoardColumn) =>
    async (wipLimit: number | undefined): Promise<void> => {
      await boardCall(
        client,
        {
          case: "columnSetLimit",
          value: { projectId: project.id, columnKey: column.key, wipLimit },
        },
        "board",
      );
      refresh();
    };
  const createBoard = async (next: NewBoard): Promise<void> => {
    const result = await boardCall(
      client,
      { case: "boardCreate", value: { projectId: project.id, ...next } },
      "structure",
    );
    if (result.outcome.case === "applied") {
      adopt(result.outcome.value);
    }
    setSheet(null);
    select(result.boardId);
  };
  const newItem = canControl ? () => setSheet("item") : undefined;
  const { ui } = moves;

  return (
    <div className="board-view">
      <BoardToolbar
        boards={model.boards}
        board={board}
        onSelect={select}
        onNewBoard={canStructure ? () => setSheet("board") : undefined}
        showArchived={showArchived}
        onShowArchived={() => setShowArchived(true)}
        onNewItem={newItem}
        onSettings={
          board === undefined
            ? undefined
            : () => {
                setOpened(null);
                setSettings(settings === null ? "general" : null);
              }
        }
        settingsOpen={settings !== null}
        switcherOpen={props.initial?.switcher}
        filters={
          <BoardFilterBar
            filters={prefs.filters}
            onChange={(filters) => updatePrefs({ ...prefs, filters })}
            botOptions={botOptions}
            shown={shown.length}
            total={cards.length}
            hiddenNames={columns.filter((c) => !c.visible).map((c) => c.name)}
            showHidden={prefs.showHidden}
            onShowHidden={(showHidden) => updatePrefs({ ...prefs, showHidden })}
          />
        }
        drafts={
          <DraftsTray
            api={client}
            drafts={drafts}
            bots={bots}
            onOpen={openItem}
            initiallyOpen={props.initial?.drafts}
          />
        }
      />
      {board === undefined ? null : (
        <LeadNotice
          projectId={project.id}
          boardId={board.id}
          proposals={model.proposals}
          who={who}
          now={now}
          onView={() => setSettings("columns")}
        />
      )}
      {cards.length === 0 ? (
        <div className="board-empty" role="status">
          <h2>No items yet</h2>
          <p>
            {`When the team plans work for ${board?.name ?? project.name}, its items show up here, column by column.`}
            {newItem === undefined ? "" : " Or start one yourself with + New item."}
          </p>
        </div>
      ) : null}
      <BoardColumns
        columns={columns}
        cards={cards}
        prefs={prefs}
        onPrefs={updatePrefs}
        gated={board === undefined || board.flow === "release_gated"}
        botsById={botsById}
        moves={moves}
        canMove={canControl}
        onOpen={openItem}
        onSetLimit={model.canRule ? setLimit : undefined}
        onNewItem={newItem}
      />
      <BoardDrawer
        api={client}
        client={client}
        openId={opened}
        shown={shown}
        drafts={model.drafts}
        columns={columns}
        bots={bots}
        canControl={canControl}
        connected={props.connected}
        leadId={project.lead_bot_id ?? undefined}
        onOpen={openItem}
        onMove={canControl ? moves.openMenu : null}
        onDraftDone={(message) => {
          setOpened(null);
          addToast("info", message, "");
        }}
      >
        <ItemOwnerActions {...props} projectId={project.id} itemId={opened} />
      </BoardDrawer>
      {settings === null || board === undefined ? null : (
        <BoardSettings
          key={board.id}
          api={client}
          board={board}
          columns={columns}
          counts={
            new Map(columns.map((c) => [c.key, cards.filter((x) => x.columnKey === c.key).length]))
          }
          canEdit={canStructure}
          proposals={model.proposals}
          who={who}
          adopt={adopt}
          onClose={() => setSettings(null)}
          initialSection={settings}
          initiallyExpanded={props.initial?.expanded}
          now={now}
        />
      )}
      {sheet === "board" ? (
        <NewBoardSheet
          boards={model.boards}
          onCreate={createBoard}
          onCancel={() => setSheet(null)}
        />
      ) : null}
      {sheet === "item" ? (
        <NewItemDialog
          api={client}
          projectId={project.id}
          board={board}
          onCreated={(id) => {
            setSheet(null);
            openItem(id);
          }}
          onCancel={() => setSheet(null)}
        />
      ) : null}
      {ui.kind === "menu" ? (
        <MoveMenu
          card={ui.card}
          anchor={ui.anchor}
          columns={columns}
          checks={ui.checks}
          onChoose={(toKey) => moves.requestMove(ui.card, toKey, ui.anchor)}
          onClose={moves.close}
        />
      ) : null}
      {ui.kind === "refused" ? (
        <RefusalPopover
          card={ui.card}
          to={ui.to}
          anchor={ui.anchor}
          unmet={ui.unmet}
          onClose={moves.close}
        />
      ) : null}
      {ui.kind === "input" ? (
        <MoveDialog
          card={ui.card}
          to={ui.to}
          needsReason={ui.needsReason}
          needsOverride={ui.needsOverride}
          unmet={ui.unmet}
          onConfirm={moves.confirm}
          onCancel={moves.close}
        />
      ) : null}
    </div>
  );
}

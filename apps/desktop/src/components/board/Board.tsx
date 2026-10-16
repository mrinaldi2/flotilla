// One project's board, live (H-018 §3; 0.19.0): the board picked in the
// switcher (remembered per project; a link to an item opens its own board),
// its toolbar, the lead's latest change, its columns, and on the right the
// item drawer or the board's settings.

import { useState } from "react";
import type { ReactElement } from "react";
import type { AddToast } from "../../app/useToasts";
import type { DaemonApi } from "../../protocol/api";
import { boardCall } from "../../protocol/board";
import type { Bot, Project } from "../../protocol/entities";
import type { BoardColumn } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { BoardSnapshot } from "../../protocol/gen/flotilla/board/v1/requests_pb";
import { useNow } from "../control/useNow";
import { ItemOwnerActions } from "../ownerActions/OwnerActionList";
import BoardColumns from "./BoardColumns";
import BoardDrawer from "./BoardDrawer";
import BoardFilterBar from "./BoardFilterBar";
import type { BoardModel } from "./boardSync";
import BoardToolbar from "./BoardToolbar";
import DraftsTray from "./drafts/DraftsTray";
import NewItemDialog from "./drafts/NewItemDialog";
import LeadNotice from "./LeadNotice";
import { MovePopups } from "./MovePopovers";
import NewBoardSheet from "./NewBoardSheet";
import type { NewBoard } from "./NewBoardSheet";
import BoardSettings from "./settings/BoardSettings";
import type { SettingsSection } from "./settings/BoardSettings";
import { useBoardMoves } from "./useBoardMoves";
import type { BoardMoves } from "./useBoardMoves";
import { useBoardView } from "./useBoardView";
import type { BoardView } from "./useBoardView";

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
    readonly discuss?: string;
  };
  /** "Now", for ages; stories pin it. */
  readonly now?: number;
}

type Sheet = "board" | "item" | null;

/** New board and New item, over the board. */
function Sheets(props: {
  readonly sheet: Sheet;
  readonly board: BoardProps;
  readonly view: BoardView;
  readonly onClose: () => void;
}): ReactElement | null {
  const { client, project, model, adopt } = props.board;
  const { view } = props;
  const createBoard = async (next: NewBoard): Promise<void> => {
    const result = await boardCall(
      client,
      { case: "boardCreate", value: { projectId: project.id, ...next } },
      "structure",
    );
    if (result.outcome.case === "applied") {
      adopt(result.outcome.value);
    }
    props.onClose();
    view.select(result.boardId);
  };
  if (props.sheet === "board") {
    return <NewBoardSheet boards={model.boards} onCreate={createBoard} onCancel={props.onClose} />;
  }
  if (props.sheet === "item") {
    return (
      <NewItemDialog
        api={client}
        projectId={project.id}
        board={view.board}
        onCreated={(id) => {
          props.onClose();
          view.openItem(id);
        }}
        onCancel={props.onClose}
      />
    );
  }
  return null;
}

/** The empty board: how its items arrive. */
function NoItems(props: { readonly name: string; readonly canAdd: boolean }): ReactElement {
  return (
    <div className="board-empty" role="status">
      <h2>No items yet</h2>
      <p>
        {`When the team plans work for ${props.name}, its items show up here, column by column.`}
        {props.canAdd ? " Or start one yourself with + New item." : ""}
      </p>
    </div>
  );
}

interface Parts {
  readonly props: BoardProps;
  readonly view: BoardView;
  readonly canStructure: boolean;
  readonly now: number;
}

/** The toolbar: switcher, filters, New item, drafts tray, ⚙ Board. */
function Bar(
  p: Parts & {
    readonly onSheet: (sheet: Sheet) => void;
    readonly newItem: (() => void) | undefined;
  },
): ReactElement {
  const { props, view } = p;
  const { prefs, updatePrefs, board, columns } = view;
  const [showArchived, setShowArchived] = useState(false);
  return (
    <BoardToolbar
      boards={props.model.boards}
      board={board}
      onSelect={view.select}
      onNewBoard={p.canStructure ? () => p.onSheet("board") : undefined}
      showArchived={showArchived}
      onShowArchived={() => setShowArchived(true)}
      onNewItem={p.newItem}
      onSettings={
        board === undefined
          ? undefined
          : () => view.setSettings(view.settings === null ? "general" : null)
      }
      settingsOpen={view.settings !== null}
      switcherOpen={props.initial?.switcher}
      filters={
        <BoardFilterBar
          filters={prefs.filters}
          onChange={(filters) => updatePrefs({ ...prefs, filters })}
          botOptions={view.botOptions}
          labelGroups={view.labelGroups}
          shown={view.shown.length}
          total={view.cards.length}
          hiddenNames={columns.filter((c) => !c.visible).map((c) => c.name)}
          showHidden={prefs.showHidden}
          onShowHidden={(showHidden) => updatePrefs({ ...prefs, showHidden })}
        />
      }
      drafts={
        <DraftsTray
          api={props.client}
          drafts={[...props.model.drafts.values()]}
          bots={props.bots}
          onOpen={view.openItem}
          initiallyOpen={props.initial?.drafts}
        />
      }
    />
  );
}

/** The right side: the open item's drawer, or the board's settings. */
function Side(p: Parts & { readonly moves: BoardMoves }): ReactElement {
  const { props, view } = p;
  const { client, project, model, canControl } = props;
  const { board, columns, cards } = view;
  const counts = new Map(
    columns.map((c) => [c.key, cards.filter((x) => x.columnKey === c.key).length]),
  );
  return (
    <>
      <BoardDrawer
        api={client}
        client={client}
        openId={view.opened}
        shown={view.shown}
        drafts={model.drafts}
        columns={columns}
        bots={props.bots}
        canControl={canControl}
        connected={props.connected}
        leadId={project.lead_bot_id ?? undefined}
        onOpen={view.openItem}
        onMove={canControl ? p.moves.openMenu : null}
        now={props.now}
        initialDiscuss={props.initial?.discuss}
        onDraftDone={(message) => {
          view.openItem(null);
          props.addToast("info", message, "");
        }}
      >
        <ItemOwnerActions {...props} projectId={project.id} itemId={view.opened} />
      </BoardDrawer>
      {view.settings === null || board === undefined ? null : (
        <BoardSettings
          key={board.id}
          api={client}
          board={board}
          columns={columns}
          counts={counts}
          canEdit={p.canStructure}
          proposals={model.proposals}
          who={view.who}
          adopt={props.adopt}
          onClose={() => view.setSettings(null)}
          initialSection={view.settings}
          initiallyExpanded={props.initial?.expanded}
          now={p.now}
        />
      )}
    </>
  );
}

export default function Board(props: BoardProps): ReactElement {
  const { client, project, model, bots, canControl, addToast, refresh } = props;
  const view = useBoardView({
    projectId: project.id,
    model,
    bots,
    openItem: props.openItem,
    openBoard: props.openBoard,
    settings: props.initial?.settings,
  });
  const { prefs, updatePrefs, board, columns, cards } = view;
  const [sheet, setSheet] = useState<Sheet>(props.initial?.sheet ?? null);
  const clock = useNow();
  const parts: Parts = {
    props,
    view,
    canStructure: model.canRule && client.hasGrant("approve"),
    now: props.now ?? clock,
  };
  const moves = useBoardMoves({ api: client, columns, refresh, addToast });
  // The owner's call, on the board's home (H-099).
  const setLimit =
    (column: BoardColumn) =>
    async (wipLimit: number | undefined): Promise<void> => {
      const value = { projectId: project.id, columnKey: column.key, wipLimit };
      await boardCall(client, { case: "columnSetLimit", value }, "board");
      refresh();
    };
  const newItem = canControl ? () => setSheet("item") : undefined;

  return (
    <div className="board-view">
      <Bar {...parts} onSheet={setSheet} newItem={newItem} />
      {board === undefined ? null : (
        <LeadNotice
          projectId={project.id}
          boardId={board.id}
          proposals={model.proposals}
          who={view.who}
          now={parts.now}
          onView={() => view.setSettings("columns")}
        />
      )}
      {cards.length === 0 ? (
        <NoItems name={board?.name ?? project.name} canAdd={newItem !== undefined} />
      ) : null}
      <BoardColumns
        columns={columns}
        cards={cards}
        prefs={prefs}
        onPrefs={updatePrefs}
        gated={board === undefined || board.flow === "release_gated"}
        botsById={view.botsById}
        moves={moves}
        canMove={canControl}
        onOpen={view.openItem}
        onSetLimit={model.canRule ? setLimit : undefined}
        onNewItem={newItem}
        chipGroups={view.chipGroups}
      />
      <Side {...parts} moves={moves} />
      <Sheets sheet={sheet} board={props} view={view} onClose={() => setSheet(null)} />
      <MovePopups moves={moves} columns={columns} />
    </div>
  );
}

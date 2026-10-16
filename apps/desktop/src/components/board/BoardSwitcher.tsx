// The board switcher at the start of the board toolbar (0.19.0): the
// project's boards, the default first, then "Archived boards…" and "New
// board…". The Board tab remembers the last board per project.

import type { ReactElement } from "react";
import type { Board } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { archivedBoards, liveBoards } from "./boards";
import MenuButton from "./MenuButton";
import type { MenuEntry } from "./MenuButton";

interface BoardSwitcherProps {
  readonly boards: readonly Board[];
  readonly current: Board;
  readonly onSelect: (boardId: string) => void;
  /** Absent when this connection can't create boards. */
  readonly onNew?: () => void;
  /** Shows the archived boards in the menu. */
  readonly showArchived: boolean;
  readonly onShowArchived: () => void;
  readonly initiallyOpen?: boolean;
}

function boardEntry(board: Board, current: Board, onSelect: (id: string) => void): MenuEntry {
  return {
    key: board.id,
    label: board.name,
    note: [board.key, board.isDefault ? "default" : "", board.archived ? "archived" : ""]
      .filter(Boolean)
      .join(" · "),
    checked: board.id === current.id,
    onChoose: () => onSelect(board.id),
  };
}

export default function BoardSwitcher(props: BoardSwitcherProps): ReactElement {
  const { current, onSelect } = props;
  const archived = archivedBoards(props.boards);
  const entries: MenuEntry[] = liveBoards(props.boards).map((b) =>
    boardEntry(b, current, onSelect),
  );
  if (props.showArchived) {
    entries.push(
      ...archived.map((board, index) => ({
        ...boardEntry(board, current, onSelect),
        separated: index === 0,
      })),
    );
  } else if (archived.length > 0) {
    entries.push({
      key: "archived",
      label: `Archived boards… (${archived.length})`,
      separated: true,
      onChoose: props.onShowArchived,
    });
  }
  if (props.onNew !== undefined) {
    entries.push({
      key: "new",
      label: "New board…",
      separated: archived.length === 0 || props.showArchived,
      onChoose: props.onNew,
    });
  }
  return (
    <MenuButton
      className="board-switcher"
      label={
        <>
          <span className="board-switcher-name">{current.name}</span>
          {current.archived ? <span className="board-switcher-note"> · archived</span> : null}
        </>
      }
      ariaLabel={`Board: ${current.name}. Switch board`}
      menuLabel="Boards"
      entries={entries}
      initiallyOpen={props.initiallyOpen}
    />
  );
}

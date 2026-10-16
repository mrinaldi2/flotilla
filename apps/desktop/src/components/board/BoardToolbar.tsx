// The board toolbar (0.19.0): the board switcher first, the filters, then
// "+ New item", the drafts tray and "⚙ Board" at the right.

import { Settings } from "lucide-react";
import type { ReactElement, ReactNode } from "react";
import type { Board } from "../../protocol/gen/flotilla/board/v1/board_pb";
import BoardSwitcher from "./BoardSwitcher";

interface BoardToolbarProps {
  readonly boards: readonly Board[];
  /** Unset from a home before 0.19: no switcher, no settings. */
  readonly board: Board | undefined;
  readonly onSelect: (boardId: string) => void;
  readonly onNewBoard?: () => void;
  readonly showArchived: boolean;
  readonly onShowArchived: () => void;
  readonly onNewItem?: () => void;
  readonly onSettings?: () => void;
  readonly settingsOpen: boolean;
  /** The filter bar. */
  readonly filters: ReactNode;
  /** The drafts tray. */
  readonly drafts: ReactNode;
  readonly switcherOpen?: boolean;
}

export default function BoardToolbar(props: BoardToolbarProps): ReactElement {
  const { board } = props;
  return (
    <div className="board-toolbar">
      {board === undefined ? null : (
        <BoardSwitcher
          boards={props.boards}
          current={board}
          onSelect={props.onSelect}
          onNew={props.onNewBoard}
          showArchived={props.showArchived}
          onShowArchived={props.onShowArchived}
          initiallyOpen={props.switcherOpen}
        />
      )}
      <div className="board-toolbar-filters">{props.filters}</div>
      <div className="board-toolbar-actions">
        {props.onNewItem === undefined ? null : (
          <button type="button" className="btn btn-small" onClick={props.onNewItem}>
            + New item
          </button>
        )}
        {props.drafts}
        {props.onSettings === undefined ? null : (
          <button
            type="button"
            className={`btn btn-small${props.settingsOpen ? " board-settings-on" : ""}`}
            aria-pressed={props.settingsOpen}
            onClick={props.onSettings}
          >
            <Settings size={12} aria-hidden="true" /> Board
          </button>
        )}
      </div>
    </div>
  );
}

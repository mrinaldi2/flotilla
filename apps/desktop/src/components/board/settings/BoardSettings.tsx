// Board settings (⚙ Board, 0.19.0): a right-side panel the width of the
// item drawer, with General, Columns, Automations and Item types. Every save
// is one board change, checked against the board version read; a refusal is
// shown at the foot in the Flotilla service's words.

import { X } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";
import { useDrawerEscape } from "../../../hooks/useDrawerEscape";
import { useNow } from "../../control/useNow";
import type { BoardApi } from "../../../protocol/board";
import type { Board, BoardColumn } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type {
  BoardProposal,
  BoardSnapshot,
} from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import AutomationsSection from "./AutomationsSection";
import type { ColumnChange } from "./columnEdits";
import ColumnsSection from "./ColumnsSection";
import type { ColumnsActions } from "./ColumnsSection";
import GeneralSection from "./GeneralSection";
import type { BoardChange } from "./GeneralSection";
import { lastChangeLine } from "./lastChange";
import { useStructure } from "./useStructure";

export type SettingsSection = "general" | "columns" | "automations" | "types";

const SECTIONS: readonly (readonly [SettingsSection, string])[] = [
  ["general", "General"],
  ["columns", "Columns"],
  ["automations", "Automations"],
  ["types", "Item types"],
];

interface BoardSettingsProps {
  readonly api: BoardApi;
  readonly board: Board;
  /** This board's columns, in order. */
  readonly columns: readonly BoardColumn[];
  /** Items per column key. */
  readonly counts: ReadonlyMap<string, number>;
  /** The owner's call on the board's home (the approve grant). */
  readonly canEdit: boolean;
  readonly proposals: readonly BoardProposal[];
  readonly who: (actor: string) => string;
  readonly adopt: (snapshot: BoardSnapshot) => void;
  readonly onClose: () => void;
  readonly initialSection?: SettingsSection;
  readonly initiallyExpanded?: string;
  /** "Now", for the footer's age; stories pin it. */
  readonly now?: number;
}

/** A column change as `column_update` carries it; a policy replaces all three lists. */
function columnUpdate(column: BoardColumn, change: ColumnChange) {
  const { policy, ...rest } = change;
  const lists =
    policy === undefined
      ? {}
      : {
          setPolicy: true,
          movers: [...policy.movers],
          rules: [...policy.rules],
          allowedFrom: [...policy.allowedFrom],
        };
  return { key: column.key, ...rest, ...lists };
}

function useBoardStructure(props: BoardSettingsProps) {
  const { board } = props;
  const structure = useStructure(props.api, props.adopt);
  const base = { boardId: board.id, version: board.version };
  const run = structure.run;
  const columns: ColumnsActions = {
    onChange: (column, change) => {
      void run({ case: "columnUpdate", value: { ...base, ...columnUpdate(column, change) } });
    },
    onAdd: (name, kind) => {
      void run({ case: "columnAdd", value: { ...base, name, kind } });
    },
    onRemove: (column, moveItemsTo) => {
      void run({ case: "columnRemove", value: { ...base, key: column.key, moveItemsTo } });
    },
  };
  return {
    structure,
    columns,
    updateBoard: (change: BoardChange): void => {
      void run({ case: "boardUpdate", value: { ...base, ...change } });
    },
    setAutomation: (fact: string, columnKey: string | undefined): void => {
      void run({ case: "automationSet", value: { ...base, fact, columnKey } });
    },
  };
}

function Section(props: {
  readonly section: SettingsSection;
  readonly settings: BoardSettingsProps;
  readonly disabled: boolean;
  readonly actions: ReturnType<typeof useBoardStructure>;
}): ReactElement {
  const { board, columns } = props.settings;
  const { disabled, actions } = props;
  switch (props.section) {
    case "general":
      return (
        <GeneralSection
          key={`${board.id}:${board.version}`}
          board={board}
          disabled={disabled}
          onChange={actions.updateBoard}
        />
      );
    case "columns":
      return (
        <ColumnsSection
          board={board}
          columns={columns}
          counts={props.settings.counts}
          disabled={disabled}
          initiallyExpanded={props.settings.initiallyExpanded}
          {...actions.columns}
        />
      );
    case "automations":
      return (
        <AutomationsSection
          board={board}
          columns={columns}
          disabled={disabled}
          onSet={actions.setAutomation}
        />
      );
    case "types":
      return (
        <section className="settings-section" aria-label="Item types">
          <p className="settings-note">
            <span aria-hidden="true">◇</span> Items use the built-in types (Epic, Feature, Bug,
            Spike, Chore) and their templates on every board. Types and templates of a board's own
            come with a later Flotilla service.
          </p>
        </section>
      );
  }
}

export default function BoardSettings(props: BoardSettingsProps): ReactElement {
  const { board, canEdit } = props;
  const [section, setSection] = useState<SettingsSection>(props.initialSection ?? "general");
  const actions = useBoardStructure(props);
  const { structure } = actions;
  const clock = useNow();
  useDrawerEscape(props.onClose);
  const last = lastChangeLine(props.proposals, board.id, props.who, props.now ?? clock);

  return (
    <aside className="item-drawer board-settings" aria-label={`Settings of board ${board.name}`}>
      <header className="drawer-head">
        <span className="drawer-type">
          <span aria-hidden="true">⚙ </span>
          {board.name} <span className="mono">{board.key}</span>
        </span>
        <button
          type="button"
          className="drawer-close"
          aria-label="Close board settings"
          onClick={props.onClose}
        >
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <div className="drawer-tabs" role="tablist" aria-label="Board settings">
        {SECTIONS.map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={section === id}
            className={section === id ? "on" : ""}
            onClick={() => setSection(id)}
          >
            {label}
          </button>
        ))}
      </div>
      <div className="drawer-body" role="tabpanel">
        {canEdit ? null : (
          <p className="settings-note" role="note">
            <span aria-hidden="true">◌</span> Read only: the owner changes a board's structure, on
            the computer that is the board's home.
          </p>
        )}
        <Section section={section} settings={props} disabled={!canEdit} actions={actions} />
      </div>
      <footer className="drawer-foot settings-foot">
        {structure.error === null ? (
          <span role="status">{structure.busy ? "Saving…" : last}</span>
        ) : (
          <span className="field-error" role="alert">
            <span aria-hidden="true">✗ </span>
            {structure.error}
          </span>
        )}
      </footer>
    </aside>
  );
}

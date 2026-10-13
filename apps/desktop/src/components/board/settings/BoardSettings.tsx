// Board settings (⚙ Board, 0.19.0): a right-side panel the width of the
// item drawer, with General, Columns, Automations and Item types. Every save
// is one board change, checked against the board version read; a refusal is
// shown at the foot in the Hermes service's words.

import { X } from "lucide-react";
import { useCallback, useState } from "react";
import type { ReactElement } from "react";
import { useDrawerEscape } from "../../../hooks/useDrawerEscape";
import { useNow } from "../../control/useNow";
import type { BoardApi } from "../../../protocol/board";
import type { Board, BoardColumn } from "../../../protocol/gen/hermes/board/v1/board_pb";
import type {
  BoardProposal,
  BoardSnapshot,
} from "../../../protocol/gen/hermes/board/v1/requests_pb";
import AutomationsSection from "./AutomationsSection";
import type { ColumnChange } from "./ColumnRow";
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

export default function BoardSettings(props: BoardSettingsProps): ReactElement {
  const { api, board, canEdit } = props;
  const [section, setSection] = useState<SettingsSection>(props.initialSection ?? "general");
  const structure = useStructure(api, props.adopt);
  const clock = useNow();
  useDrawerEscape(props.onClose);
  const base = { boardId: board.id, version: board.version };
  const disabled = !canEdit || structure.busy;

  const updateBoard = (change: BoardChange): void => {
    void structure.run({ case: "boardUpdate", value: { ...base, ...change } });
  };
  const columns: ColumnsActions = {
    onChange: (column: BoardColumn, change: ColumnChange) => {
      const { policy, ...rest } = change;
      void structure.run({
        case: "columnUpdate",
        value: {
          ...base,
          key: column.key,
          ...rest,
          ...(policy === undefined
            ? {}
            : {
                setPolicy: true,
                movers: [...policy.movers],
                rules: [...policy.rules],
                allowedFrom: [...policy.allowedFrom],
              }),
        },
      });
    },
    onAdd: (name, kind) => {
      void structure.run({ case: "columnAdd", value: { ...base, name, kind } });
    },
    onRemove: (column, moveItemsTo) => {
      void structure.run({
        case: "columnRemove",
        value: { ...base, key: column.key, moveItemsTo },
      });
    },
  };
  const setAutomation = useCallback(
    (fact: string, columnKey: string | undefined): void => {
      void structure.run({
        case: "automationSet",
        value: { boardId: board.id, version: board.version, fact, columnKey },
      });
    },
    [structure, board.id, board.version],
  );
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
        {section === "general" ? (
          <GeneralSection
            key={`${board.id}:${board.version}`}
            board={board}
            disabled={disabled}
            onChange={updateBoard}
          />
        ) : null}
        {section === "columns" ? (
          <ColumnsSection
            board={board}
            columns={props.columns}
            counts={props.counts}
            disabled={disabled}
            initiallyExpanded={props.initiallyExpanded}
            {...columns}
          />
        ) : null}
        {section === "automations" ? (
          <AutomationsSection
            board={board}
            columns={props.columns}
            disabled={disabled}
            onSet={setAutomation}
          />
        ) : null}
        {section === "types" ? (
          <section className="settings-section" aria-label="Item types">
            <p className="settings-note">
              <span aria-hidden="true">◇</span> Items use the built-in types (Epic, Feature, Bug,
              Spike, Chore) and their templates on every board. Types and templates of a board's own
              come with a later Hermes service.
            </p>
          </section>
        ) : null}
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

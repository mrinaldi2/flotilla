// Board settings › General (0.19.0): name, key (fixed: items carry it),
// description, the default board, the structure policy and archiving.

import { useState } from "react";
import type { ReactElement } from "react";
import type { Board } from "../../../protocol/gen/flotilla/board/v1/board_pb";

export interface BoardChange {
  readonly name?: string;
  readonly description?: string;
  readonly isDefault?: boolean;
  readonly archived?: boolean;
  readonly structurePolicy?: string;
}

interface GeneralSectionProps {
  readonly board: Board;
  readonly disabled: boolean;
  readonly onChange: (change: BoardChange) => void;
}

const POLICIES: readonly {
  readonly value: string;
  readonly word: string;
  readonly note: string;
}[] = [
  {
    value: "owner_confirms",
    word: "You confirm",
    note: "The lead proposes columns, rules and automations; each waits for you in Needs you.",
  },
  {
    value: "lead_free",
    word: "Lead decides",
    note: "The lead changes the structure directly; you are told and can change it back.",
  },
];

export default function GeneralSection({
  board,
  disabled,
  onChange,
}: GeneralSectionProps): ReactElement {
  const [name, setName] = useState(board.name);
  const [description, setDescription] = useState(board.description);
  const edited = name.trim() !== board.name || description !== board.description;
  return (
    <section className="settings-section" aria-label="General">
      <label className="field">
        <span className="field-label">Name</span>
        <input
          type="text"
          value={name}
          disabled={disabled}
          onChange={(e) => setName(e.target.value)}
        />
      </label>
      <label className="field">
        <span className="field-label">Key</span>
        <input type="text" value={board.key} readOnly aria-describedby="board-key-note" />
        <span id="board-key-note" className="field-hint">
          Items carry it in their ids ({board.key}-1), so it stays as it is.
        </span>
      </label>
      <label className="field">
        <span className="field-label">Description</span>
        <textarea
          rows={3}
          value={description}
          disabled={disabled}
          onChange={(e) => setDescription(e.target.value)}
        />
      </label>
      {disabled ? null : (
        <div className="settings-actions">
          <button
            type="button"
            className="btn btn-small btn-primary"
            disabled={!edited || name.trim() === ""}
            onClick={() =>
              onChange({
                ...(name.trim() === board.name ? {} : { name: name.trim() }),
                ...(description === board.description ? {} : { description }),
              })
            }
          >
            Save
          </button>
        </div>
      )}
      <fieldset className="settings-group" disabled={disabled}>
        <legend className="field-label">Structure changes by the lead</legend>
        {POLICIES.map((p) => (
          <label key={p.value} className="settings-radio">
            <input
              type="radio"
              name="structure-policy"
              checked={board.structurePolicy === p.value}
              onChange={() => onChange({ structurePolicy: p.value })}
            />
            {p.word}
            <span className="settings-radio-note">{p.note}</span>
          </label>
        ))}
        <p className="field-hint">
          Renames, order, WIP limits and visibility always apply directly.
        </p>
      </fieldset>
      <div className="settings-group">
        <span className="field-label">Default board</span>
        {board.isDefault ? (
          <p className="settings-line">
            <span aria-hidden="true">★</span> New items and links without a board land here.
          </p>
        ) : (
          <button
            type="button"
            className="btn btn-small"
            disabled={disabled || board.archived}
            onClick={() => onChange({ isDefault: true })}
          >
            Make this the default board
          </button>
        )}
      </div>
      <div className="settings-group">
        <span className="field-label">Archive</span>
        {board.isDefault ? (
          <p className="settings-line">
            The default board can't be archived; make another board the default first.
          </p>
        ) : (
          <button
            type="button"
            className={`btn btn-small${board.archived ? "" : " btn-danger"}`}
            disabled={disabled}
            onClick={() => onChange({ archived: !board.archived })}
          >
            {board.archived ? "Restore this board" : "Archive this board"}
          </button>
        )}
      </div>
    </section>
  );
}

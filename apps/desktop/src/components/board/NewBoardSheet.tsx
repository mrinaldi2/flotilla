// "New board" (0.19.0): a name, a key suggested from it and checked as you
// type, and a preset shown with its columns. Create opens the new board.

import { useState } from "react";
import type { FormEvent, ReactElement } from "react";
import type { Board } from "../../protocol/gen/hermes/board/v1/board_pb";
import { useEscapeKey } from "../../hooks/useEscapeKey";
import { errText } from "../../util";
import OverlayShell from "../overlay/OverlayShell";
import { keyProblem, suggestKey } from "./boards";
import { PRESETS } from "./presets";
import type { Preset, PresetName } from "./presets";
import { kindChip } from "./settings/rules";

export interface NewBoard {
  readonly name: string;
  readonly key: string;
  readonly preset: PresetName;
}

interface NewBoardSheetProps {
  readonly boards: readonly Board[];
  /** Resolves once the board exists; rejects with the Hermes service's refusal. */
  readonly onCreate: (board: NewBoard) => Promise<void>;
  readonly onCancel: () => void;
  /** Prefills the form (stories). */
  readonly initial?: Partial<NewBoard>;
}

function PresetPreview({ preset }: { readonly preset: Preset }): ReactElement {
  return (
    <ol className="preset-columns" aria-label={`${preset.title} columns`}>
      {preset.columns.map((column) => (
        <li key={column.name} className={column.hidden === true ? "preset-hidden" : ""}>
          <span className="preset-column-name">{column.name}</span>
          <span className="preset-column-kind">{kindChip(column.kind)}</span>
          {column.note === undefined ? null : (
            <span className="preset-column-note">{column.note}</span>
          )}
          {column.hidden === true ? <span className="preset-column-note">hidden</span> : null}
        </li>
      ))}
    </ol>
  );
}

function PresetPicker(props: {
  readonly value: PresetName;
  readonly onChange: (preset: PresetName) => void;
}): ReactElement {
  return (
    <fieldset className="new-board-presets">
      <legend className="field-label">Preset</legend>
      {PRESETS.map((p) => (
        <label key={p.name} className={`preset${props.value === p.name ? " preset-on" : ""}`}>
          <span className="preset-head">
            <input
              type="radio"
              name="preset"
              value={p.name}
              checked={props.value === p.name}
              onChange={() => props.onChange(p.name)}
            />
            <span className="preset-title">{p.title}</span>
            <span className="preset-blurb">{p.blurb}</span>
          </span>
          <PresetPreview preset={p} />
        </label>
      ))}
    </fieldset>
  );
}

/** The key's hint: its problem, or what ids it gives. */
function keyHint(key: string, problem: string | null): string {
  if (problem !== null) {
    return problem;
  }
  return `Items get ids like ${key === "" ? "KEY" : key}-1.`;
}

function NameAndKey(props: {
  readonly name: string;
  readonly keyText: string;
  readonly problem: string | null;
  readonly onName: (name: string) => void;
  readonly onKey: (key: string) => void;
}): ReactElement {
  const { problem } = props;
  return (
    <>
      <div className="field-row">
        <label className="field">
          <span className="field-label">Name</span>
          <input
            type="text"
            value={props.name}
            autoFocus
            placeholder="Recipes"
            onChange={(event) => props.onName(event.target.value)}
          />
        </label>
        <label className="field new-board-key">
          <span className="field-label">Key</span>
          <input
            type="text"
            value={props.keyText}
            aria-describedby="new-board-key-hint"
            aria-invalid={problem !== null}
            onChange={(event) => props.onKey(event.target.value)}
          />
        </label>
      </div>
      <p id="new-board-key-hint" className={problem === null ? "field-hint" : "field-error"}>
        {keyHint(props.keyText, problem)}
      </p>
    </>
  );
}

export default function NewBoardSheet(props: NewBoardSheetProps): ReactElement {
  const [name, setName] = useState(props.initial?.name ?? "");
  // Typed by the owner; until then it follows the name.
  const [typedKey, setTypedKey] = useState<string | null>(props.initial?.key ?? null);
  const [preset, setPreset] = useState<PresetName>(props.initial?.preset ?? "simple");
  const [sending, setSending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  useEscapeKey(props.onCancel);
  const named = name.trim() !== "";
  const key = typedKey ?? (named ? suggestKey(name) : "");
  const problem = named ? keyProblem(key, props.boards) : null;
  const ready = named && problem === null && !sending;

  const submit = (event: FormEvent): void => {
    event.preventDefault();
    if (!ready) {
      return;
    }
    setSending(true);
    setRefusal(null);
    props.onCreate({ name: name.trim(), key, preset }).catch((failure: unknown) => {
      setRefusal(errText(failure));
      setSending(false);
    });
  };

  return (
    <OverlayShell label="New board" onClose={props.onCancel}>
      <form className="confirm-dialog new-board" onSubmit={submit}>
        <h2 className="confirm-title">New board</h2>
        <NameAndKey
          name={name}
          keyText={key}
          problem={problem}
          onName={setName}
          onKey={(typed) => setTypedKey(typed.toUpperCase())}
        />
        <PresetPicker value={preset} onChange={setPreset} />
        {refusal === null ? null : (
          <p className="field-error" role="alert">
            {refusal}
          </p>
        )}
        <div className="confirm-actions">
          <button type="button" className="btn btn-small" onClick={props.onCancel}>
            Cancel
          </button>
          <button type="submit" className="btn btn-small btn-primary" disabled={!ready}>
            {sending ? "Creating…" : "Create board"}
          </button>
        </div>
      </form>
    </OverlayShell>
  );
}

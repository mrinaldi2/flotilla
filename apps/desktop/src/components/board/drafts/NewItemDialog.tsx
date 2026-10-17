// "+ New item" (0.19.0): a title and a type, saved as a draft on the board
// shown, which then opens in the drawer to be worked out (alone, or with a
// bot) and finalized into a column. From 0.21.0 the types are Task, Design,
// Bug and Spike, and on an initiative's board the card belongs to a node of
// its plan.

import { useState } from "react";
import type { FormEvent, ReactElement } from "react";
import { useDialogKeys } from "../../../hooks/useReturnFocus";
import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { Board } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { ItemType } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { PlanNode } from "../../../protocol/gen/flotilla/board/v1/plan_pb";
import { ITEM_TYPES_V2 } from "../../../protocol/plan";
import { errText } from "../../../util";
import OverlayShell from "../../overlay/OverlayShell";
import { nodeOptions, optionLabel, preferredNode } from "../../plan/belongsTo";
import { typesFor } from "../edit/itemEdit";
import { typeHint, typeLabel } from "../labels";

interface NewItemDialogProps {
  readonly api: BoardApi;
  readonly projectId: string;
  /** The board it goes on; the project's default without boards. */
  readonly board: Board | undefined;
  /** The project's plan nodes, when planning is on: a card on an initiative's board joins one. */
  readonly planNodes?: readonly PlanNode[];
  readonly onCreated: (itemId: string) => void;
  readonly onCancel: () => void;
}

/** The card types, each with what it is for. */
function TypeChoices(props: {
  readonly types: readonly ItemType[];
  readonly type: ItemType;
  readonly onType: (type: ItemType) => void;
}): ReactElement {
  return (
    <fieldset className="new-item-types">
      <legend className="field-label">Type</legend>
      {props.types.map((t) => (
        <label key={t} className="settings-radio new-item-type">
          <input
            type="radio"
            name="new-item-type"
            checked={props.type === t}
            onChange={() => props.onType(t)}
          />
          <span>
            {`${typeLabel(t).glyph} ${typeLabel(t).word}`}
            {typeHint(t) === "" ? null : <span className="field-hint">{typeHint(t)}</span>}
          </span>
        </label>
      ))}
    </fieldset>
  );
}

/** "Belongs to": the plan node on an initiative's board; nothing elsewhere. */
function BelongsTo(props: {
  readonly options: readonly PlanNode[];
  readonly node: string | undefined;
  readonly onNode: (id: string) => void;
}): ReactElement | null {
  if (props.options.length === 0) {
    return null;
  }
  return (
    <label className="field">
      <span className="field-label">Belongs to</span>
      <select value={props.node ?? ""} onChange={(e) => props.onNode(e.target.value)}>
        {props.options.map((n) => (
          <option key={n.id} value={n.id}>
            {optionLabel(n)}
          </option>
        ))}
      </select>
      <span className="field-hint">
        Tasks belong to a feature in Build. Spikes and design tickets can belong to an epic or
        initiative in research.
      </span>
    </label>
  );
}

export default function NewItemDialog(props: NewItemDialogProps): ReactElement {
  const [title, setTitle] = useState("");
  const types = typesFor(props.api.capabilities);
  const [type, setType] = useState<ItemType>(types[0] ?? ItemType.TASK);
  const options = nodeOptions(props.planNodes ?? [], props.board?.id);
  const [picked, setPicked] = useState<string | undefined>(undefined);
  const node = picked ?? preferredNode(options, type);
  const [sending, setSending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  useDialogKeys(props.onCancel);
  const submit = (event: FormEvent): void => {
    event.preventDefault();
    if (title.trim() === "" || sending) {
      return;
    }
    setSending(true);
    setRefusal(null);
    const value = {
      projectId: props.projectId,
      type,
      title: title.trim(),
      boardId: props.board?.id,
      draft: true,
      planNodeId: options.length === 0 ? undefined : node,
    };
    void (async () => {
      try {
        const result = await boardCall(props.api, { case: "itemCreate", value }, "edited");
        setSending(false);
        if (result.outcome.case === "done") {
          props.onCreated(result.outcome.value.id);
        } else if (result.outcome.case === "refused") {
          setRefusal(result.outcome.value.unmet.map((u) => u.text).join(" "));
        }
      } catch (failure) {
        setSending(false);
        setRefusal(errText(failure));
      }
    })();
  };
  return (
    <OverlayShell label="New item" onClose={props.onCancel}>
      <form className="confirm-dialog new-item" onSubmit={submit}>
        <h2 className="confirm-title">
          New item{props.board === undefined ? "" : ` on ${props.board.name}`}
        </h2>
        <p className="confirm-body">
          It starts as a draft: only you, and the bots you discuss it with, see it until you
          finalize it into a column.
        </p>
        <label className="field">
          <span className="field-label">Title</span>
          <input type="text" value={title} autoFocus onChange={(e) => setTitle(e.target.value)} />
        </label>
        <TypeChoices types={types} type={type} onType={setType} />
        <BelongsTo options={options} node={node} onNode={setPicked} />
        {props.api.capabilities.includes(ITEM_TYPES_V2) ? (
          <p className="new-item-note">
            Looking for Feature or Epic? Features and epics are created in the Plan, through their
            parent&apos;s approved document.
          </p>
        ) : null}
        {refusal === null ? null : (
          <p className="field-error" role="alert">
            {refusal}
          </p>
        )}
        <div className="confirm-actions">
          <button type="button" className="btn btn-small" onClick={props.onCancel}>
            Cancel
          </button>
          <button
            type="submit"
            className="btn btn-small btn-primary"
            disabled={title.trim() === "" || sending}
          >
            {sending ? "Saving…" : "Save as draft"}
          </button>
        </div>
      </form>
    </OverlayShell>
  );
}

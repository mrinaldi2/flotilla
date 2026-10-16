// "+ New item" (0.19.0): a title and a type, saved as a draft on the board
// shown, which then opens in the drawer to be worked out (alone, or with a
// bot) and finalized into a column.

import { useState } from "react";
import type { FormEvent, ReactElement } from "react";
import { useDialogKeys } from "../../../hooks/useReturnFocus";
import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { Board } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { ItemType } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { errText } from "../../../util";
import OverlayShell from "../../overlay/OverlayShell";
import { TYPES } from "../edit/itemEdit";
import { typeLabel } from "../labels";

interface NewItemDialogProps {
  readonly api: BoardApi;
  readonly projectId: string;
  /** The board it goes on; the project's default without boards. */
  readonly board: Board | undefined;
  readonly onCreated: (itemId: string) => void;
  readonly onCancel: () => void;
}

export default function NewItemDialog(props: NewItemDialogProps): ReactElement {
  const [title, setTitle] = useState("");
  const [type, setType] = useState(ItemType.FEATURE);
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
        <fieldset className="new-item-types">
          <legend className="field-label">Type</legend>
          {TYPES.map((t) => (
            <label key={t} className="settings-radio">
              <input
                type="radio"
                name="new-item-type"
                checked={type === t}
                onChange={() => setType(t)}
              />
              {`${typeLabel(t).glyph} ${typeLabel(t).word}`}
            </label>
          ))}
        </fieldset>
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

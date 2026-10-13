// The owner editing an item from its drawer (0.19.0): title, type,
// priority, size, assignee, labels, description and acceptance criteria.
// An image pasted into the description is attached and placed inline.
// Saved as one `item_update` (and `item_assign` when the assignee changed);
// a refusal lists its reasons, a conflict asks to look again.

import { useState } from "react";
import type { ClipboardEvent, FormEvent, ReactElement } from "react";
import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { Bot } from "../../../protocol/entities";
import type { Item } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { Priority, Size } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { errText } from "../../../util";
import { typeLabel } from "../labels";
import type { ItemForm } from "./itemEdit";
import { formOf, formProblem, PRIORITIES, SIZES, TYPES, updateFor } from "./itemEdit";

interface ItemEditFormProps {
  readonly api: BoardApi;
  readonly item: Item;
  readonly bots: readonly Bot[];
  /** Attaches a pasted image; resolves with the Markdown that shows it inline. */
  readonly onPasteImage?: (file: File) => Promise<string | null>;
  readonly onSaved: () => void;
  readonly onCancel: () => void;
}

async function save(api: BoardApi, item: Item, form: ItemForm): Promise<string | null> {
  const update = updateFor(item, form);
  let version = item.version;
  if (update !== null) {
    const result = await boardCall(api, { case: "itemUpdate", value: update }, "edited");
    if (result.outcome.case === "refused") {
      return result.outcome.value.unmet.map((u) => u.text).join(" ");
    }
    if (result.outcome.case !== "done") {
      return `${item.id} changed while you edited it. Cancel to see it as it is now.`;
    }
    version = result.outcome.value.version;
  }
  if (form.assignee !== (item.assignee ?? "")) {
    const bot = form.assignee === "" ? undefined : form.assignee;
    const result = await boardCall(
      api,
      { case: "itemAssign", value: { id: item.id, expectedVersion: version, bot } },
      "edited",
    );
    if (result.outcome.case === "refused") {
      return result.outcome.value.unmet.map((u) => u.text).join(" ");
    }
  }
  return null;
}

export default function ItemEditForm(props: ItemEditFormProps): ReactElement {
  const { item } = props;
  const [form, setForm] = useState<ItemForm>(() => formOf(item));
  const [sending, setSending] = useState(false);
  const [refusal, setRefusal] = useState<string | null>(null);
  const problem = formProblem(form);
  const set = (change: Partial<ItemForm>): void => setForm((f) => ({ ...f, ...change }));

  const submit = (event: FormEvent): void => {
    event.preventDefault();
    if (problem !== null || sending) {
      return;
    }
    setSending(true);
    setRefusal(null);
    void (async () => {
      let refused: string | null;
      try {
        refused = await save(props.api, item, form);
      } catch (failure) {
        refused = errText(failure);
      }
      setSending(false);
      if (refused === null) {
        props.onSaved();
      } else {
        setRefusal(refused);
      }
    })();
  };

  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>): void => {
    const file = [...event.clipboardData.files].find((f) => f.type.startsWith("image/"));
    const paste = props.onPasteImage;
    if (file === undefined || paste === undefined) {
      return;
    }
    event.preventDefault();
    const area = event.currentTarget;
    const at = area.selectionStart;
    void (async () => {
      const markdown = await paste(file);
      if (markdown !== null) {
        setForm((f) => ({
          ...f,
          description: `${f.description.slice(0, at)}${markdown}${f.description.slice(at)}`,
        }));
      }
    })();
  };

  return (
    <form className="item-edit" onSubmit={submit} aria-label={`Edit ${item.id}`}>
      <label className="field">
        <span className="field-label">Title</span>
        <input
          type="text"
          value={form.title}
          autoFocus
          onChange={(e) => set({ title: e.target.value })}
        />
      </label>
      <div className="field-row">
        <label className="field">
          <span className="field-label">Type</span>
          <select value={form.type} onChange={(e) => set({ type: Number(e.target.value) })}>
            {TYPES.map((t) => (
              <option key={t} value={t}>{`${typeLabel(t).glyph} ${typeLabel(t).word}`}</option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field-label">Priority</span>
          <select value={form.priority} onChange={(e) => set({ priority: Number(e.target.value) })}>
            {PRIORITIES.map((p) => (
              <option key={p} value={p}>
                {Priority[p]}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field-label">Size</span>
          <select
            value={form.size ?? ""}
            onChange={(e) =>
              set({ size: e.target.value === "" ? undefined : Number(e.target.value) })
            }
          >
            {item.size === undefined ? <option value="">—</option> : null}
            {SIZES.map((s) => (
              <option key={s} value={s}>
                {Size[s]}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="field-row">
        <label className="field">
          <span className="field-label">Assignee</span>
          <select value={form.assignee} onChange={(e) => set({ assignee: e.target.value })}>
            <option value="">Unassigned</option>
            {props.bots.map((b) => (
              <option key={b.id} value={b.id}>
                {b.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span className="field-label">Labels</span>
          <input
            type="text"
            value={form.labels}
            placeholder="area:web, ui"
            onChange={(e) => set({ labels: e.target.value })}
          />
        </label>
      </div>
      <label className="field">
        <span className="field-label">Description</span>
        <textarea
          rows={8}
          value={form.description}
          onPaste={onPaste}
          onChange={(e) => set({ description: e.target.value })}
        />
        <span className="field-hint">Markdown. Paste a screenshot to attach it here.</span>
      </label>
      <label className="field">
        <span className="field-label">Acceptance criteria</span>
        <textarea
          rows={4}
          value={form.criteria}
          onChange={(e) => set({ criteria: e.target.value })}
        />
        <span className="field-hint">
          One per line. A criterion kept word for word keeps its check.
        </span>
      </label>
      {(problem ?? refusal) ? (
        <p className="field-error" role="alert">
          {problem ?? refusal}
        </p>
      ) : null}
      <div className="confirm-actions">
        <button type="button" className="btn btn-small" onClick={props.onCancel}>
          Cancel
        </button>
        <button
          type="submit"
          className="btn btn-small btn-primary"
          disabled={problem !== null || sending}
        >
          {sending ? "Saving…" : "Save"}
        </button>
      </div>
    </form>
  );
}

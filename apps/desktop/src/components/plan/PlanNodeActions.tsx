// What the owner does to a plan node (0.21.0): move a goal along (define →
// active → met, `plan_update` with its stage), edit a node's title,
// summary, deadline and lead (`plan_update`), and drop it with its subtree
// (`plan_drop`). Each answers with the node, a refusal in the guards'
// words, or a conflict when the node moved meanwhile, said in place.

import { useState } from "react";
import type { FormEvent, ReactElement } from "react";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { useDialogKeys } from "../../hooks/useReturnFocus";
import type { BoardApi } from "../../protocol/board";
import type { Bot } from "../../protocol/entities";
import type { PlanNode, PlanUpdateSchema } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { PlanKind, PlanStage } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import OverlayShell from "../overlay/OverlayShell";
import { outcomeLine, planEdit } from "./planEdits";
import type { PlanOutcome } from "./planEdits";
import { isFinished, kindWords } from "./planText";

type Update = MessageInitShape<typeof PlanUpdateSchema>;

/** A goal's next stage by hand, and its button; none for any other node. */
export function goalMove(
  node: PlanNode,
): { readonly stage: PlanStage; readonly label: string } | null {
  if (node.kind !== PlanKind.GOAL) {
    return null;
  }
  if (node.stage === PlanStage.DEFINE) {
    return { stage: PlanStage.ACTIVE, label: "Make active" };
  }
  if (node.stage === PlanStage.ACTIVE) {
    return { stage: PlanStage.MET, label: "Mark met" };
  }
  return null;
}

export interface NodeForm {
  readonly title: string;
  readonly summary: string;
  readonly due: string;
  /** A bot id, or "" for the project's lead. */
  readonly lead: string;
}

export function formOf(node: PlanNode): NodeForm {
  return { title: node.title, summary: node.summary, due: node.due ?? "", lead: node.lead ?? "" };
}

/** The `plan_update` for what changed; null when nothing did. "" clears a deadline or a lead. */
export function updateOf(node: PlanNode, form: NodeForm): Update | null {
  const title = form.title.trim();
  const due = form.due.trim();
  const changes = {
    ...(title === node.title ? {} : { title }),
    ...(form.summary === node.summary ? {} : { summary: form.summary }),
    ...(node.kind !== PlanKind.GOAL || due === (node.due ?? "") ? {} : { due }),
    ...(node.kind === PlanKind.GOAL || form.lead === (node.lead ?? "") ? {} : { lead: form.lead }),
  };
  if (Object.keys(changes).length === 0) {
    return null;
  }
  return { id: node.id, expectedVersion: node.version, ...changes };
}

/** Sends one edit; says what stands in the way, or nothing once it is done. */
function useSend(onChanged: (node: PlanNode) => void, title: string) {
  const [sending, setSending] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const send = async (edit: () => Promise<PlanOutcome>): Promise<boolean> => {
    setSending(true);
    setSaid(null);
    const outcome = await edit();
    setSending(false);
    setSaid(outcomeLine(outcome, title));
    if (outcome.kind === "done" || outcome.kind === "conflict") {
      onChanged(outcome.node);
    }
    return outcome.kind === "done";
  };
  return { sending, said, send };
}

interface ActionProps {
  readonly api: BoardApi;
  readonly node: PlanNode;
  readonly bots: readonly Bot[];
  readonly leadName?: string;
  readonly onChanged: (node: PlanNode) => void;
}

function EditForm(props: ActionProps & { readonly onDone: () => void }): ReactElement {
  const { node } = props;
  const [form, setForm] = useState<NodeForm>(() => formOf(node));
  const { sending, said, send } = useSend(props.onChanged, node.title);
  const set = (change: Partial<NodeForm>): void => setForm((f) => ({ ...f, ...change }));
  const goal = node.kind === PlanKind.GOAL;
  const submit = (event: FormEvent): void => {
    event.preventDefault();
    const value = updateOf(node, form);
    if (value === null) {
      props.onDone();
      return;
    }
    if (form.title.trim() === "" || sending) {
      return;
    }
    void (async () => {
      if (await send(() => planEdit(props.api, { case: "planUpdate", value }))) {
        props.onDone();
      }
    })();
  };
  return (
    <form className="plan-edit" onSubmit={submit} aria-label={`Edit ${node.title}`}>
      <label className="field">
        <span className="field-label">Title</span>
        <input
          type="text"
          value={form.title}
          autoFocus
          onChange={(e) => set({ title: e.target.value })}
        />
      </label>
      <label className="field">
        <span className="field-label">{goal ? "Outcome and measure" : "Summary"}</span>
        <textarea
          rows={4}
          value={form.summary}
          onChange={(e) => set({ summary: e.target.value })}
        />
      </label>
      {goal ? (
        <label className="field">
          <span className="field-label">Deadline</span>
          <input type="text" value={form.due} onChange={(e) => set({ due: e.target.value })} />
        </label>
      ) : (
        <label className="field">
          <span className="field-label">Lead</span>
          <select value={form.lead} onChange={(e) => set({ lead: e.target.value })}>
            <option value="">
              {props.leadName === undefined
                ? "The project's lead"
                : `The project's lead (${props.leadName})`}
            </option>
            {props.bots.map((b) => (
              <option key={b.id} value={b.id}>
                {b.name}
              </option>
            ))}
          </select>
        </label>
      )}
      {said === null ? null : (
        <p className="field-error" role="alert">
          {said}
        </p>
      )}
      <div className="plan-gate-actions">
        <button
          type="submit"
          className="btn btn-small btn-primary"
          disabled={sending || form.title.trim() === ""}
        >
          {sending ? "Saving…" : "Save"}
        </button>
        <button type="button" className="btn btn-small" onClick={props.onDone}>
          Cancel
        </button>
      </div>
    </form>
  );
}

function DropDialog(props: ActionProps & { readonly onClose: () => void }): ReactElement {
  const { node } = props;
  const [reason, setReason] = useState("");
  const { sending, said, send } = useSend(props.onChanged, node.title);
  useDialogKeys(props.onClose);
  const word = kindWords(node.kind).word.toLowerCase();
  const drop = (): void => {
    const value = {
      id: node.id,
      expectedVersion: node.version,
      reason: reason.trim() === "" ? undefined : reason.trim(),
    };
    void (async () => {
      if (await send(() => planEdit(props.api, { case: "planDrop", value }))) {
        props.onClose();
      }
    })();
  };
  return (
    <OverlayShell label={`Drop ${node.title}`} onClose={props.onClose}>
      <div className="confirm-dialog">
        <h2 className="confirm-title">{`Drop the ${word} ${node.title}?`}</h2>
        <p className="confirm-body">
          It stops, and so does everything under it that isn&apos;t finished. Their cards stay on
          the board.
        </p>
        <label className="field">
          <span className="field-label">Why (optional)</span>
          <textarea rows={2} value={reason} onChange={(e) => setReason(e.target.value)} />
        </label>
        {said === null ? null : (
          <p className="field-error" role="alert">
            {said}
          </p>
        )}
        <div className="confirm-actions">
          <button type="button" className="btn btn-small" autoFocus onClick={props.onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-small btn-danger"
            disabled={sending}
            onClick={drop}
          >
            {`Drop the ${word}`}
          </button>
        </div>
      </div>
    </OverlayShell>
  );
}

/** A goal's next stage, Edit and Drop…, as this connection may. */
export default function PlanNodeActions(
  props: ActionProps & {
    /** The owner's: a goal's stage, and Drop. */
    readonly canRule: boolean;
    /** The control grant: Edit. */
    readonly canEdit: boolean;
    /** Stories: open on the form or the dialog. */
    readonly initial?: "edit" | "drop";
  },
): ReactElement | null {
  const { node } = props;
  const [open, setOpen] = useState<"edit" | "drop" | null>(props.initial ?? null);
  const stage = useSend(props.onChanged, node.title);
  const move = props.canRule ? goalMove(node) : null;
  const live = !isFinished(node);
  if (open === "edit") {
    return <EditForm {...props} onDone={() => setOpen(null)} />;
  }
  if (move === null && !(props.canEdit && live) && !(props.canRule && live)) {
    return null;
  }
  return (
    <div className="plan-actions">
      {move === null ? null : (
        <button
          type="button"
          className="btn btn-small btn-primary"
          disabled={stage.sending}
          onClick={() => {
            const value = { id: node.id, expectedVersion: node.version, stage: move.stage };
            void stage.send(() => planEdit(props.api, { case: "planUpdate", value }));
          }}
        >
          {move.label}
        </button>
      )}
      {props.canEdit && live ? (
        <button type="button" className="btn btn-small" onClick={() => setOpen("edit")}>
          Edit
        </button>
      ) : null}
      {props.canRule && live ? (
        <button type="button" className="btn btn-small" onClick={() => setOpen("drop")}>
          Drop…
        </button>
      ) : null}
      {stage.said === null ? null : (
        <p className="field-error" role="alert">
          {stage.said}
        </p>
      )}
      {open === "drop" ? <DropDialog {...props} onClose={() => setOpen(null)} /> : null}
    </div>
  );
}

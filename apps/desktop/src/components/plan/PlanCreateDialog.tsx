// + Goal and + Initiative (0.21.0): the owner starts a goal with its
// outcome and deadline, or an initiative under a goal with the "why" that
// seeds its research and the bot that leads it. Starting an initiative makes
// its board; it begins in research. `plan_create`, answered with `planned`.

import { useState } from "react";
import type { FormEvent, ReactElement } from "react";
import { useDialogKeys } from "../../hooks/useReturnFocus";
import type { BoardApi } from "../../protocol/board";
import type { Bot } from "../../protocol/entities";
import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { PlanKind } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import OverlayShell from "../overlay/OverlayShell";
import { outcomeLine, planEdit } from "./planEdits";
import { isFinished } from "./planText";

export type CreateKind = "goal" | "initiative";

interface PlanCreateDialogProps {
  readonly api: BoardApi;
  readonly projectId: string;
  readonly kind: CreateKind;
  /** The plan's nodes: an initiative goes under one of its goals. */
  readonly nodes: readonly PlanNode[];
  readonly bots: readonly Bot[];
  /** The project's lead, by name: an initiative's lead unless another is picked. */
  readonly leadName?: string;
  /** The goal to put an initiative under at first. */
  readonly goalId?: string;
  readonly onCreated: (node: PlanNode) => void;
  readonly onCancel: () => void;
}

interface Fields {
  readonly title: string;
  readonly summary: string;
  readonly due: string;
  readonly goal: string;
  readonly lead: string;
}

function trimmed(text: string): string | undefined {
  const value = text.trim();
  return value === "" ? undefined : value;
}

const WORDS: Readonly<
  Record<CreateKind, { readonly body: string; readonly summary: string; readonly hint: string }>
> = {
  goal: {
    body: "What the project is for, and how you'll know it's met. You write it; initiatives serve it.",
    summary: "Outcome and measure",
    hint: "Markdown, e.g. the number you want to move.",
  },
  initiative: {
    body: "It gets its own board and starts in research. Its lead writes the PRD; you approve it.",
    summary: "Why",
    hint: "It seeds the research brief. Markdown.",
  },
};

/** The `plan_create` for the fields: a goal's deadline, an initiative's goal and lead. */
function createOf(props: PlanCreateDialogProps, fields: Fields) {
  const initiative = props.kind === "initiative";
  return {
    projectId: props.projectId,
    kind: initiative ? PlanKind.INITIATIVE : PlanKind.GOAL,
    title: fields.title.trim(),
    parentId: initiative ? fields.goal : undefined,
    summary: trimmed(fields.summary),
    due: initiative ? undefined : trimmed(fields.due),
    lead: initiative ? trimmed(fields.lead) : undefined,
  };
}

function useCreate(props: PlanCreateDialogProps, goals: readonly PlanNode[]) {
  const [fields, setFields] = useState<Fields>({
    title: "",
    summary: "",
    due: "",
    goal: props.goalId ?? goals[0]?.id ?? "",
    lead: "",
  });
  const [sending, setSending] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const ready = fields.title.trim() !== "" && (props.kind === "goal" || fields.goal !== "");
  const submit = (event: FormEvent): void => {
    event.preventDefault();
    if (!ready || sending) {
      return;
    }
    setSending(true);
    setSaid(null);
    const value = createOf(props, fields);
    void (async () => {
      const outcome = await planEdit(props.api, { case: "planCreate", value });
      setSending(false);
      if (outcome.kind === "done") {
        props.onCreated(outcome.node);
      } else {
        setSaid(outcomeLine(outcome, value.title));
      }
    })();
  };
  const set = (change: Partial<Fields>): void => setFields((f) => ({ ...f, ...change }));
  return { fields, set, ready, sending, said, submit };
}

type FieldProps = {
  readonly fields: Fields;
  readonly set: (change: Partial<Fields>) => void;
};

/** An initiative's goal. */
function GoalPicker(props: FieldProps & { readonly goals: readonly PlanNode[] }): ReactElement {
  return (
    <label className="field">
      <span className="field-label">Goal</span>
      <select value={props.fields.goal} onChange={(e) => props.set({ goal: e.target.value })}>
        {props.goals.length === 0 ? <option value="">Start a goal first</option> : null}
        {props.goals.map((g) => (
          <option key={g.id} value={g.id}>
            {g.title}
          </option>
        ))}
      </select>
    </label>
  );
}

/** An initiative's lead: the project's unless another is picked. */
function LeadPicker(
  props: FieldProps & { readonly bots: readonly Bot[]; readonly leadName?: string },
): ReactElement {
  return (
    <label className="field">
      <span className="field-label">Lead</span>
      <select value={props.fields.lead} onChange={(e) => props.set({ lead: e.target.value })}>
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
  );
}

/** A goal's deadline, in the owner's words. */
function Deadline(props: FieldProps): ReactElement {
  return (
    <label className="field">
      <span className="field-label">Deadline</span>
      <input
        type="text"
        value={props.fields.due}
        placeholder="Playtest round 2"
        onChange={(e) => props.set({ due: e.target.value })}
      />
      <span className="field-hint">In your words; optional.</span>
    </label>
  );
}

export default function PlanCreateDialog(props: PlanCreateDialogProps): ReactElement {
  const goals = props.nodes.filter((n) => n.kind === PlanKind.GOAL && !isFinished(n));
  const c = useCreate(props, goals);
  useDialogKeys(props.onCancel);
  const initiative = props.kind === "initiative";
  const words = WORDS[props.kind];
  return (
    <OverlayShell label={`New ${props.kind}`} onClose={props.onCancel}>
      <form className="confirm-dialog plan-create" onSubmit={c.submit}>
        <h2 className="confirm-title">{`New ${props.kind}`}</h2>
        <p className="confirm-body">{words.body}</p>
        {initiative ? <GoalPicker fields={c.fields} set={c.set} goals={goals} /> : null}
        <label className="field">
          <span className="field-label">Title</span>
          <input
            type="text"
            value={c.fields.title}
            autoFocus
            onChange={(e) => c.set({ title: e.target.value })}
          />
        </label>
        <label className="field">
          <span className="field-label">{words.summary}</span>
          <textarea
            rows={3}
            value={c.fields.summary}
            onChange={(e) => c.set({ summary: e.target.value })}
          />
          <span className="field-hint">{words.hint}</span>
        </label>
        {initiative ? (
          <LeadPicker fields={c.fields} set={c.set} bots={props.bots} leadName={props.leadName} />
        ) : (
          <Deadline fields={c.fields} set={c.set} />
        )}
        {c.said === null ? null : (
          <p className="field-error" role="alert">
            {c.said}
          </p>
        )}
        <div className="confirm-actions">
          <button type="button" className="btn btn-small" onClick={props.onCancel}>
            Cancel
          </button>
          <button
            type="submit"
            className="btn btn-small btn-primary"
            disabled={!c.ready || c.sending}
          >
            {c.sending ? "Starting…" : `Start the ${props.kind}`}
          </button>
        </div>
      </form>
    </OverlayShell>
  );
}

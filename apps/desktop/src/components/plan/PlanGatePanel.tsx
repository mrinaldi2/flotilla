// The owner's ruling on a node's gate (0.21.0, canvas D2): what was
// submitted, what is already settled, the children an approval creates (each
// can be left out), and a note to the lead, needed to send it back. A
// proposed initiative is accepted or declined; the lead's request to skip
// research is granted or refused. The owner also skips open research with a
// reason, recorded on the node.

import { useState } from "react";
import type { CSSProperties, ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { PlanGate, PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import {
  PlanDocKind,
  PlanResearchState,
  PlanStage,
} from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { day } from "./planHistory";
import { leftOut, outcomeLine, planEdit } from "./planEdits";
import type { PlanOutcome } from "./planEdits";
import { fileName, kindWords, stageWord } from "./planText";

interface GateWords {
  readonly question: string;
  readonly approve: string;
  readonly sendBack: string;
  /** The heading of the children it creates, and what happens to them. */
  readonly children?: { readonly legend: string; readonly hint: string };
}

/** What the panel asks for each kind of gate. */
export function gateWords(gate: PlanGate): GateWords {
  switch (gate.stage) {
    case PlanStage.PROPOSED:
      return {
        question: "Accept the proposed initiative?",
        approve: "Accept",
        sendBack: "Decline",
      };
    case PlanStage.RESEARCH:
      return { question: "Skip research?", approve: "Skip research", sendBack: "Keep research" };
    case PlanStage.PRD:
      return {
        question: "Approve the PRD?",
        approve: "Approve",
        sendBack: "Send back",
        children: { legend: "Epics it creates", hint: "Each starts in research." },
      };
    case PlanStage.HLD:
      return {
        question: "Approve the HLD?",
        approve: "Approve",
        sendBack: "Send back",
        children: { legend: "Features it creates", hint: "Each starts in LLD." },
      };
    case PlanStage.LLD:
      return {
        question: "Approve the LLD?",
        approve: "Approve",
        sendBack: "Send back",
        children: {
          legend: "Tasks it creates",
          hint: "Each is a card on the initiative's board, in its first open column.",
        },
      };
    default:
      return {
        question: `Approve the ${stageWord(gate.stage)}?`,
        approve: "Approve",
        sendBack: "Send back",
      };
  }
}

/** "second submission", "submission 3": which round this is, after the first. */
export function roundWords(round: number): string {
  if (round <= 1) {
    return "";
  }
  return round === 2 ? "second submission" : `submission ${round}`;
}

/** What is already settled before this gate: research, and an epic's approved design. */
function Settled({ node }: { readonly node: PlanNode }): ReactElement | null {
  const lines: string[] = [];
  const research = node.research;
  if (research?.state === PlanResearchState.CLOSED) {
    const n = research.spikes.length;
    lines.push(`Research closed · ${n} ${n === 1 ? "spike" : "spikes"} with documents`);
  } else if (research?.state === PlanResearchState.SKIPPED) {
    lines.push(`Research skipped: ${research.skippedReason ?? "no reason given"}`);
  }
  const design = node.docs.find((d) => d.kind === PlanDocKind.DESIGN && d.approvedAt);
  if (node.gate?.stage === PlanStage.HLD && design !== undefined) {
    lines.push(`Design approved ${day(design.approvedAt)}`);
  }
  if (lines.length === 0) {
    return null;
  }
  return (
    <ul className="plan-settled">
      {lines.map((line) => (
        <li key={line}>
          <span aria-hidden="true">✓</span> {line}
        </li>
      ))}
    </ul>
  );
}

interface PanelProps {
  readonly api: BoardApi;
  readonly node: PlanNode;
  readonly gate: PlanGate;
  /** The owner's: the approve grant. */
  readonly canRule: boolean;
  readonly who: (actor: string) => string;
  /** The node after a ruling. */
  readonly onRuled: (node: PlanNode) => void;
  /** Its width, which the owner drags (0.22.0). */
  readonly style?: CSSProperties;
}

function useRuling(props: PanelProps) {
  const { api, node, gate } = props;
  const [kept, setKept] = useState<readonly boolean[]>(() => gate.children.map(() => true));
  const [note, setNote] = useState("");
  const [sending, setSending] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const rule = (approve: boolean): void => {
    if (sending || (!approve && note.trim() === "")) {
      return;
    }
    setSending(true);
    setSaid(null);
    const value = {
      id: node.id,
      expectedVersion: node.version,
      approve,
      note: note.trim() === "" ? undefined : note.trim(),
      leaveOut: approve ? leftOut(kept) : [],
    };
    void (async () => {
      const outcome: PlanOutcome = await planEdit(api, { case: "planRule", value });
      setSending(false);
      setSaid(outcomeLine(outcome, node.title));
      if (outcome.kind === "done" || outcome.kind === "conflict") {
        props.onRuled(outcome.node);
      }
    })();
  };
  return { kept, setKept, note, setNote, sending, said, rule };
}

export default function PlanGatePanel(props: PanelProps): ReactElement {
  const { gate, node } = props;
  const words = gateWords(gate);
  const r = useRuling(props);
  const creating = r.kept.filter(Boolean).length;
  const by = [props.who(gate.submittedBy), day(gate.submittedAt), roundWords(gate.round)];
  return (
    <aside className="plan-gate-panel" aria-label="Gate" style={props.style}>
      <div className="plan-gate-head">
        <h2>{words.question}</h2>
        <span className="plan-dim">{`Submitted by ${by.filter(Boolean).join(" · ")}`}</span>
      </div>
      <Settled node={node} />
      {gate.note ? (
        <blockquote className="plan-gate-note">
          <span className="plan-dim">{props.who(gate.submittedBy)} says</span>
          <span>{gate.note}</span>
        </blockquote>
      ) : null}
      {words.children !== undefined && gate.children.length > 0 ? (
        <fieldset className="plan-children">
          <legend>{words.children.legend}</legend>
          {gate.children.map((child, index) => (
            <label key={`${child.title}-${child.summary ?? ""}`} className="plan-child">
              <input
                type="checkbox"
                checked={r.kept[index] ?? true}
                disabled={!props.canRule}
                onChange={(e) => {
                  const on = e.target.checked;
                  r.setKept((was) => was.map((k, i) => (i === index ? on : k)));
                }}
              />
              <span>
                <span className="plan-child-title">{child.title}</span>
                {child.summary ? <span className="plan-dim">{child.summary}</span> : null}
              </span>
            </label>
          ))}
          <span className="plan-dim">{`${words.children.hint} Untick one to leave it out.`}</span>
        </fieldset>
      ) : null}
      {props.canRule ? (
        <>
          <label className="field">
            <span className="field-label">{`Note to the lead (needed to ${words.sendBack.toLowerCase()})`}</span>
            <textarea
              rows={3}
              value={r.note}
              placeholder="What should change?"
              onChange={(e) => r.setNote(e.target.value)}
            />
          </label>
          <div className="plan-gate-actions">
            <button
              type="button"
              className="btn btn-primary"
              disabled={r.sending}
              onClick={() => r.rule(true)}
            >
              {words.children !== undefined && gate.children.length > 0
                ? `${words.approve} · create ${creating}`
                : words.approve}
            </button>
            <button
              type="button"
              className="btn"
              disabled={r.sending || r.note.trim() === ""}
              onClick={() => r.rule(false)}
            >
              {words.sendBack}
            </button>
          </div>
        </>
      ) : (
        <p className="plan-dim">Waiting for the owner to decide.</p>
      )}
      {r.said === null ? null : (
        <p className="field-error" role="alert">
          {r.said}
        </p>
      )}
      {gate.sha && gate.docPath ? (
        <p className="plan-dim plan-gate-pin">
          {`Approval is pinned to ${gate.sha}. If ${fileName(gate.docPath)} changes after that, the ${kindWords(node.kind).word.toLowerCase()} shows “changed since approval” and the lead resubmits.`}
        </p>
      ) : null}
    </aside>
  );
}

/** "Skip research…": the owner closes open research with a reason, recorded on the node. */
export function SkipResearch(props: {
  readonly api: BoardApi;
  readonly node: PlanNode;
  readonly onSkipped: (node: PlanNode) => void;
}): ReactElement {
  const { api, node } = props;
  const [open, setOpen] = useState(false);
  const [reason, setReason] = useState("");
  const [sending, setSending] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const skip = (): void => {
    if (sending || reason.trim() === "") {
      return;
    }
    setSending(true);
    setSaid(null);
    const value = { id: node.id, expectedVersion: node.version, reason: reason.trim() };
    void (async () => {
      const outcome = await planEdit(api, { case: "planResearchSkip", value });
      setSending(false);
      setSaid(outcomeLine(outcome, node.title));
      if (outcome.kind === "done" || outcome.kind === "conflict") {
        props.onSkipped(outcome.node);
      }
      if (outcome.kind === "done") {
        setOpen(false);
      }
    })();
  };
  if (!open) {
    return (
      <button type="button" className="btn btn-small" onClick={() => setOpen(true)}>
        Skip research…
      </button>
    );
  }
  return (
    <div className="plan-skip">
      <label className="field">
        <span className="field-label">Why no research is needed</span>
        <textarea rows={2} value={reason} onChange={(e) => setReason(e.target.value)} />
        <span className="field-hint">Recorded on the node, with your name.</span>
      </label>
      <div className="plan-gate-actions">
        <button
          type="button"
          className="btn btn-small btn-primary"
          disabled={sending || reason.trim() === ""}
          onClick={skip}
        >
          Skip research
        </button>
        <button type="button" className="btn btn-small" onClick={() => setOpen(false)}>
          Cancel
        </button>
      </div>
      {said === null ? null : (
        <p className="field-error" role="alert">
          {said}
        </p>
      )}
    </div>
  );
}

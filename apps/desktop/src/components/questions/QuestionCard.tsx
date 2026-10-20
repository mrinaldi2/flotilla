// One question for the owner (0.22.0), wherever it is answered: the card's
// drawer, a plan node, the project's Questions and Needs you. An open one
// shows its options (the bot's recommendation marked), a reply box, Answer
// and Dismiss with a reason; an answered one shows the answer. Answering
// names the version read, so a question changed meanwhile comes back as it
// is now, with nothing written.

import { useState } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { QuestionState } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import type { QuestionOutcome } from "../../protocol/questions";
import { answerQuestion, dismissQuestion } from "../../protocol/questions";
import { errText } from "../../util";
import { answeredLine, askedLine, optionLabel, refusalText } from "./questionText";

export interface QuestionCardProps {
  readonly api: BoardApi;
  readonly question: Question;
  /** The approve grant: answers and dismisses. */
  readonly canAnswer: boolean;
  /** Takes the question as an answer or a conflict left it. */
  readonly onChanged: (question: Question) => void;
  /** Say which card or node it is on: lists that mix them. */
  readonly showWhere?: boolean;
  /** Puts the reply box in focus: a row asked to answer it. */
  readonly autoFocus?: boolean;
}

/** What happened to the last try, in words; null while nothing went wrong. */
function noteOf(outcome: QuestionOutcome): string | null {
  if (outcome.kind === "done") {
    return null;
  }
  if (outcome.kind === "conflict") {
    return outcome.question.state === QuestionState.OPEN
      ? "It changed while you read it. Check it and answer again."
      : "It was settled meanwhile; nothing you wrote was sent.";
  }
  return refusalText(outcome.code, outcome.text);
}

function useSettle(props: QuestionCardProps) {
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const run = async (send: () => Promise<QuestionOutcome>): Promise<boolean> => {
    setBusy(true);
    setNote(null);
    try {
      const outcome = await send();
      setNote(noteOf(outcome));
      if (outcome.kind !== "refused") {
        props.onChanged(outcome.question);
      }
      return outcome.kind === "done";
    } catch (failure) {
      setNote(errText(failure));
      return false;
    } finally {
      setBusy(false);
    }
  };
  return { busy, note, run };
}

function Options(props: {
  readonly question: Question;
  readonly picked: string | undefined;
  readonly onPick?: (key: string | undefined) => void;
}): ReactElement | null {
  const q = props.question;
  if (q.options.length === 0) {
    return null;
  }
  const pick = props.onPick;
  return (
    <div className="question-options" role="group" aria-label="Options">
      {q.options.map((o) => {
        const recommended = q.recommendation === o.key;
        const on = props.picked === o.key;
        return (
          <button
            key={o.key}
            type="button"
            className={`question-option${on ? " question-option-on" : ""}${recommended ? " question-option-rec" : ""}`}
            aria-pressed={pick === undefined ? undefined : on}
            disabled={pick === undefined}
            title={o.description}
            onClick={() => pick?.(on ? undefined : o.key)}
          >
            <span className="question-option-label">{o.label}</span>
            {recommended ? <span className="question-option-tag">Recommended</span> : null}
            {o.description === undefined ? null : (
              <span className="question-option-desc">{o.description}</span>
            )}
          </button>
        );
      })}
    </div>
  );
}

function DismissForm(props: {
  readonly busy: boolean;
  readonly onDismiss: (reason: string) => void;
  readonly onCancel: () => void;
}): ReactElement {
  const [reason, setReason] = useState("");
  return (
    <div className="question-dismiss">
      <input
        type="text"
        aria-label="Why dismiss it"
        placeholder="Why it needs no answer"
        value={reason}
        autoFocus
        onChange={(e) => setReason(e.target.value)}
      />
      <button
        type="button"
        className="btn btn-small"
        disabled={props.busy || reason.trim() === ""}
        onClick={() => props.onDismiss(reason.trim())}
      >
        Dismiss
      </button>
      <button type="button" className="btn btn-small" onClick={props.onCancel}>
        Cancel
      </button>
    </div>
  );
}

function AnswerForm(props: QuestionCardProps): ReactElement {
  const q = props.question;
  const [picked, setPicked] = useState<string | undefined>(undefined);
  const [text, setText] = useState("");
  const [dismissing, setDismissing] = useState(false);
  const { busy, note, run } = useSettle(props);
  const ready = picked !== undefined || text.trim() !== "";
  const answer = (): void => {
    void run(() => answerQuestion(props.api, q, { option: picked, text: text.trim() }));
  };
  return (
    <>
      <Options question={q} picked={picked} onPick={setPicked} />
      <textarea
        className="question-reply"
        aria-label={`Your answer to: ${q.text}`}
        placeholder={q.options.length > 0 ? "Add a note (optional)" : "Your answer"}
        rows={2}
        value={text}
        autoFocus={props.autoFocus}
        onChange={(e) => setText(e.target.value)}
      />
      {dismissing ? (
        <DismissForm
          busy={busy}
          onCancel={() => setDismissing(false)}
          onDismiss={(reason) => void run(() => dismissQuestion(props.api, q, reason))}
        />
      ) : (
        <div className="question-actions">
          <button
            type="button"
            className="btn btn-small btn-primary"
            disabled={busy || !ready}
            onClick={answer}
          >
            {busy ? "Sending…" : "Answer"}
          </button>
          <button type="button" className="btn btn-small" onClick={() => setDismissing(true)}>
            Dismiss…
          </button>
        </div>
      )}
      {note === null ? null : (
        <p className="question-note" role="alert">
          {note}
        </p>
      )}
    </>
  );
}

/** An answered or dismissed question: what the owner said. */
function Settled({ question: q }: { readonly question: Question }): ReactElement {
  if (q.state === QuestionState.DISMISSED) {
    return (
      <div className="question-answer">
        <p>
          <b>Dismissed</b>
          {q.dismissedReason ? `: ${q.dismissedReason}` : ""}
        </p>
      </div>
    );
  }
  const picked = optionLabel(q, q.answer?.option);
  const text = q.answer?.text ?? "";
  return (
    <div className="question-answer">
      <p>
        {picked === "" ? null : <b>{picked}</b>}
        {picked !== "" && text !== "" ? " · " : null}
        {text}
      </p>
      <p className="question-meta">
        {answeredLine(q)}
        {q.writtenTo === undefined ? "" : ` · added to ${q.writtenTo}`}
      </p>
    </div>
  );
}

export default function QuestionCard(props: QuestionCardProps): ReactElement {
  const q = props.question;
  const open = q.state === QuestionState.OPEN;
  return (
    <li
      className={`question${open ? " question-open" : " question-settled"}`}
      aria-label={`Question: ${q.text}`}
    >
      <div className="question-head">
        <span className="question-glyph" aria-hidden="true">
          ?
        </span>
        <span className="question-text">{q.text}</span>
        {open && q.blocking ? (
          <span className="question-chip" title="Research can't close nor the gate be submitted">
            Holds the gate
          </span>
        ) : null}
      </div>
      {q.why === "" ? null : <p className="question-why">{q.why}</p>}
      <p className="question-meta">{askedLine(q, props.showWhere === true)}</p>
      {!open ? (
        <Settled question={q} />
      ) : props.canAnswer ? (
        <AnswerForm {...props} />
      ) : (
        <Options question={q} picked={undefined} />
      )}
    </li>
  );
}

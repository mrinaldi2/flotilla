// What a gate panel shows before the ruling, at every step (0.24.0): the
// spikes of research waiting for the owner, the questions answered (an open
// blocking one holds the gate), and the step approving moves the node to.

import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type {
  PlanGate,
  PlanNode,
  PlanNodeDetail,
} from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { PlanDocKind } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { QuestionState } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { optionLabel } from "../questions/questionText";
import { useNodeQuestions } from "./PlanNodeQuestions";
import { nextStepWords, researchAwaits } from "./planText";

const NONE: readonly never[] = [];

/** "The ~250 celebrated saints · Start with the LEV": the owner's answer in words. */
function answerWords(q: Question): string {
  return [optionLabel(q, q.answer?.option), q.answer?.text ?? ""].filter(Boolean).join(" · ");
}

/** The spikes of research waiting for the owner, each with its document or not. */
function Spikes(props: {
  readonly node: PlanNode;
  readonly cards: readonly ItemCard[];
}): ReactElement {
  const { node } = props;
  const spikes = node.research?.spikes ?? [];
  return (
    <section className="plan-gate-facts" aria-label="Spikes">
      <h3 className="plan-gate-label">Spikes</h3>
      {spikes.length === 0 ? <p className="plan-empty">No spikes.</p> : null}
      <ul>
        {spikes.map((id) => {
          const attached = node.docs.some(
            (d) => d.kind === PlanDocKind.RESEARCH && d.itemId === id,
          );
          const title = props.cards.find((c) => c.id === id)?.title ?? "";
          return (
            <li key={id}>
              <span aria-hidden="true" className={attached ? "plan-gate-tick" : "plan-dim"}>
                {attached ? "✓" : "○"}
              </span>{" "}
              <span className="mono plan-dim">{id}</span> {title}
              <span className="plan-dim">
                {attached ? " · document attached" : " · no document yet"}
              </span>
            </li>
          );
        })}
      </ul>
    </section>
  );
}

/** The questions settled before this gate, and an open blocking one holding it. */
function Answered(props: {
  readonly api: BoardApi;
  readonly node: PlanNode;
  readonly detail: PlanNodeDetail;
}): ReactElement | null {
  const { detail } = props;
  const live = useNodeQuestions(props.api, props.node.id, detail.cards, detail.questions);
  const answered = live.questions.filter((q) => q.state === QuestionState.ANSWERED);
  const open = live.questions.filter((q) => q.state === QuestionState.OPEN);
  if (answered.length === 0 && open.length === 0) {
    return null;
  }
  const holding = open.filter((q) => q.blocking);
  return (
    <section className="plan-gate-facts" aria-label="Questions answered">
      <h3 className="plan-gate-label">Questions answered</h3>
      <p className="plan-line">{`${answered.length} answered, ${open.length} open`}</p>
      <ul>
        {holding.map((q) => (
          <li key={q.id} className="plan-gate-holds">
            <span aria-hidden="true">⚠ </span>
            {`Holds the gate: ${q.text}`}
          </li>
        ))}
        {answered.map((q) => (
          <li key={q.id} className="plan-gate-answer">
            <span>{q.text}</span>
            <span className="plan-dim">{answerWords(q)}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}

export default function PlanGateFacts(props: {
  readonly api: BoardApi;
  readonly node: PlanNode;
  readonly gate: PlanGate;
  /** The node's cards and questions, once read. */
  readonly detail: PlanNodeDetail | null;
}): ReactElement {
  const { node, gate, detail } = props;
  const next = nextStepWords(node, gate);
  return (
    <>
      {researchAwaits(node, gate) ? <Spikes node={node} cards={detail?.cards ?? NONE} /> : null}
      {detail === null ? null : <Answered api={props.api} node={node} detail={detail} />}
      {next === "" ? null : (
        <section className="plan-gate-facts" aria-label="Approving moves it to">
          <h3 className="plan-gate-label">Approving moves it to</h3>
          <p className="plan-line">{next}</p>
        </section>
      )}
    </>
  );
}

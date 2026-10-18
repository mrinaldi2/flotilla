// A plan node's questions for the owner (0.22.0): its own and its cards',
// the open ones answered in place under Research, a blocking one marked as
// holding the gate, and the answered ones folded below.

import { useCallback } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { isOpen } from "../../protocol/questions";
import QuestionCard from "../questions/QuestionCard";
import { openCount } from "../questions/questionText";
import { useLiveQuestions } from "../questions/useLiveQuestions";

export interface PlanNodeQuestionsProps {
  readonly api: BoardApi;
  readonly nodeId: string;
  /** Its cards: their questions count as the node's. */
  readonly cards: readonly ItemCard[];
  readonly questions: readonly Question[];
  /** The owner's: answers and dismisses. */
  readonly canAnswer: boolean;
}

export default function PlanNodeQuestions(props: PlanNodeQuestionsProps): ReactElement | null {
  const { nodeId, cards } = props;
  const belongs = useCallback(
    (q: Question): boolean => q.planNodeId === nodeId || cards.some((c) => c.id === q.itemId),
    [nodeId, cards],
  );
  const live = useLiveQuestions(props.api, props.questions, belongs);
  if (live.questions.length === 0) {
    return null;
  }
  const open = live.questions.filter(isOpen);
  const settled = live.questions.filter((q) => !isOpen(q));
  const card = (q: Question): ReactElement => (
    <QuestionCard
      key={q.id}
      api={props.api}
      question={q}
      canAnswer={props.canAnswer}
      onChanged={live.apply}
      showWhere={q.itemId !== undefined}
    />
  );
  const count = openCount(live.questions);
  return (
    <div className="plan-questions" aria-label="Questions">
      <h4>Questions{count === "" ? "" : ` · ${count}`}</h4>
      {open.length === 0 ? null : <ul className="question-list">{open.map(card)}</ul>}
      {settled.length === 0 ? null : (
        <details className="plan-questions-settled">
          <summary>{`${settled.length} answered`}</summary>
          <ul className="question-list">{settled.map(card)}</ul>
        </details>
      )}
    </div>
  );
}

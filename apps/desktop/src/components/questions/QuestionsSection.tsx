// A card's or a plan node's questions for the owner (0.22.0): open ones
// first, each answered in place, under a heading with the open count.

import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import QuestionCard from "./QuestionCard";
import { openCount } from "./questionText";

export default function QuestionsSection(props: {
  readonly api: BoardApi;
  readonly questions: readonly Question[];
  readonly canAnswer: boolean;
  readonly onChanged: (question: Question) => void;
  /** Say which card each is on: a node's list holds its cards'. */
  readonly showWhere?: boolean;
  /** The heading's level: a drawer's h4, a plan section's h3. */
  readonly heading?: "h3" | "h4";
}): ReactElement | null {
  if (props.questions.length === 0) {
    return null;
  }
  const Heading = props.heading ?? "h4";
  const count = openCount(props.questions);
  return (
    <section className="questions" aria-label="Questions">
      <Heading>Questions{count === "" ? "" : ` · ${count}`}</Heading>
      <ul className="question-list">
        {props.questions.map((q) => (
          <QuestionCard
            key={q.id}
            api={props.api}
            question={q}
            canAnswer={props.canAnswer}
            onChanged={props.onChanged}
            showWhere={props.showWhere}
          />
        ))}
      </ul>
    </section>
  );
}

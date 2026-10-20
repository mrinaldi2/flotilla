// Needs you's question rows (0.22.0, `questions_v1`): one per open question
// a bot asked with `ask_owner`, a blocking one marked. Answer opens the
// question under its row, read from the project's open questions, and it
// is answered there; the row leaves once the dashboard reads it again.

import { useEffect, useState } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import type { AttentionRowJson } from "../../protocol/dashboard";
import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { QuestionState } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { isOpen, listQuestions } from "../../protocol/questions";
import { errText } from "../../util";
import QuestionCard from "../questions/QuestionCard";
import type { RowViewProps } from "./DashRow";

/** Where Needs you answers questions: the project's board and whether this connection may. */
export interface QuestionAnswering {
  readonly api: BoardApi;
  readonly projectId: string;
  readonly canAnswer: boolean;
  /** An answer or a dismissal landed: read Needs you again. */
  readonly onAnswered: () => void;
}

type Found =
  | { readonly kind: "loading" }
  | { readonly kind: "found"; readonly question: Question }
  | { readonly kind: "gone"; readonly text: string };

/** The open question `id`, from its card's open questions or the project's. */
function useOpenQuestion(at: QuestionAnswering, id: string, itemId: string | undefined): Found {
  const [found, setFound] = useState<Found>({ kind: "loading" });
  const { api, projectId } = at;
  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const page = await listQuestions(
          api,
          { projectId, itemId, state: QuestionState.OPEN },
          undefined,
          200,
        );
        const question = page.questions.find((q) => q.id === id);
        if (live) {
          setFound(
            question === undefined
              ? { kind: "gone", text: "It was answered or dismissed meanwhile." }
              : { kind: "found", question },
          );
        }
      } catch (failure) {
        if (live) {
          setFound({ kind: "gone", text: errText(failure) });
        }
      }
    })();
    return () => {
      live = false;
    };
  }, [api, projectId, id, itemId]);
  return found;
}

function InlineQuestion(props: {
  readonly at: QuestionAnswering;
  readonly id: string;
  readonly itemId: string | undefined;
}): ReactElement {
  const found = useOpenQuestion(props.at, props.id, props.itemId);
  const [shown, setShown] = useState<Question | null>(null);
  if (found.kind !== "found") {
    return (
      <p className="dash-question-note" role="status">
        {found.kind === "loading" ? "Loading the question…" : found.text}
      </p>
    );
  }
  return (
    <ul className="question-list dash-question">
      <QuestionCard
        api={props.at.api}
        question={shown ?? found.question}
        canAnswer={props.at.canAnswer}
        autoFocus
        onChanged={(question) => {
          setShown(question);
          if (!isOpen(question)) {
            props.at.onAnswered();
          }
        }}
      />
    </ul>
  );
}

/** "Should v1 cover…?" with "Holds the gate · Lead asks you on ASL-4". */
export function questionRow(r: AttentionRowJson, at: QuestionAnswering | undefined): RowViewProps {
  const id = r.decision_id ?? "";
  const item = r.item_id === undefined || r.item_id === "" ? undefined : r.item_id;
  const meta = [
    r.question_blocking === true ? "Holds the gate" : "",
    `${r.bot?.name ?? "A bot"} asks you${item === undefined ? "" : ` on ${item}`}`,
  ]
    .filter(Boolean)
    .join(" · ");
  const answerable = at !== undefined && id !== "";
  return {
    glyph: "?",
    tone: "you",
    title: r.title,
    meta,
    action: answerable ? "Answer" : undefined,
    label: `Answer: ${r.title}`,
    inline: answerable ? () => <InlineQuestion at={at} id={id} itemId={item} /> : undefined,
    verb: "Answer",
  };
}

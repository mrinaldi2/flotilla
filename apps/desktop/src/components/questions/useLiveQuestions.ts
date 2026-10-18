// A list of questions as read, kept live (0.22.0): an answer the owner gave
// here and each `question_change` push for the project replace their
// question, and a new one that belongs joins at the top. A fresh read
// (another `initial`) starts over from it.

import { useCallback, useEffect, useState } from "react";
import type { BoardApi } from "../../protocol/board";
import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { openFirst, withQuestion } from "../../protocol/questions";

export interface LiveQuestions {
  /** Open ones first. */
  readonly questions: readonly Question[];
  /** Takes a question an answer or a dismissal came back with. */
  readonly apply: (question: Question) => void;
}

export function useLiveQuestions(
  api: BoardApi,
  initial: readonly Question[],
  /** Whether a pushed question belongs in this list. */
  belongs: (question: Question) => boolean,
): LiveQuestions {
  const [state, setState] = useState({ from: initial, list: initial });
  // A new read replaces what was gathered on the old one.
  const list = state.from === initial ? state.list : initial;
  if (state.from !== initial) {
    setState({ from: initial, list: initial });
  }
  const apply = useCallback(
    (question: Question): void => {
      setState((s) => ({ from: s.from, list: withQuestion(s.list, question, belongs(question)) }));
    },
    [belongs],
  );
  useEffect(() => {
    return api.onQuestionChange((change) => {
      const question = change.question;
      if (question !== undefined && belongs(question)) {
        apply(question);
      }
    });
  }, [api, apply, belongs]);
  return { questions: openFirst(list), apply };
}

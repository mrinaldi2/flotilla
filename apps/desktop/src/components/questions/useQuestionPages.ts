// A project's questions, a page at a time (0.22.0): `question_list` with the
// filter the owner picked, `next` for the page after, and each
// `question_change` push for the project kept in the list.

import { useCallback, useEffect, useState } from "react";
import type { BoardApi } from "../../protocol/board";
import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import type { QuestionFilter } from "../../protocol/questions";
import { listQuestions, matches, withQuestion } from "../../protocol/questions";
import { errText } from "../../util";

export interface QuestionPages {
  readonly questions: readonly Question[];
  readonly loading: boolean;
  readonly error: string | null;
  /** Reads the next page; absent once there is none. */
  readonly more?: () => void;
  readonly apply: (question: Question) => void;
}

interface Pages {
  readonly key: string;
  readonly questions: readonly Question[];
  readonly next?: string;
  readonly loading: boolean;
  readonly error: string | null;
}

function keyOf(filter: QuestionFilter): string {
  return JSON.stringify([filter.projectId, filter.itemId, filter.planNodeId, filter.state]);
}

export function useQuestionPages(api: BoardApi, filter: QuestionFilter): QuestionPages {
  const key = keyOf(filter);
  const [pages, setPages] = useState<Pages>({ key, questions: [], loading: true, error: null });
  const [cursor, setCursor] = useState<{ readonly key: string; readonly before?: string }>({ key });
  if (pages.key !== key) {
    setPages({ key, questions: [], loading: true, error: null });
    setCursor({ key });
  }
  const { projectId, itemId, planNodeId, state } = filter;

  useEffect(() => {
    if (cursor.key !== key) {
      return undefined;
    }
    let live = true;
    void (async () => {
      try {
        const page = await listQuestions(
          api,
          { projectId, itemId, planNodeId, state },
          cursor.before,
        );
        if (live) {
          setPages((p) => ({
            key,
            questions:
              cursor.before === undefined ? page.questions : [...p.questions, ...page.questions],
            next: page.next,
            loading: false,
            error: null,
          }));
        }
      } catch (failure) {
        if (live) {
          setPages((p) => ({ ...p, loading: false, error: errText(failure) }));
        }
      }
    })();
    return () => {
      live = false;
    };
  }, [api, key, cursor, projectId, itemId, planNodeId, state]);

  const apply = useCallback(
    (question: Question): void => {
      setPages((p) => ({
        ...p,
        questions: withQuestion(
          p.questions,
          question,
          matches(question, { projectId, itemId, planNodeId, state }),
        ),
      }));
    },
    [projectId, itemId, planNodeId, state],
  );
  useEffect(() => {
    return api.onQuestionChange((change) => {
      if (change.question !== undefined) {
        apply(change.question);
      }
    });
  }, [api, apply]);

  const next = pages.next;
  const more =
    next === undefined || pages.loading
      ? undefined
      : (): void => {
          setPages((p) => ({ ...p, loading: true }));
          setCursor({ key, before: next });
        };
  return { questions: pages.questions, loading: pages.loading, error: pages.error, more, apply };
}

// Questions for the owner (0.22.0, `questions.proto`): one record per
// question on a card or a plan node, answered once with an option, text or
// both, or dismissed with a reason. Without `questions_v1` the daemon has
// none, and nothing here is asked.

import type { BoardApi } from "./board";
import { boardCall } from "./board";
import type { Question } from "./gen/flotilla/board/v1/questions_pb";
import { QuestionState } from "./gen/flotilla/board/v1/questions_pb";

/** Questions for the owner: the requests, the push and the counts. */
export const QUESTIONS_V1 = "questions_v1";

export function questionsOn(api: BoardApi): boolean {
  return api.capabilities.includes(QUESTIONS_V1);
}

/** What an answer or a dismissal came to. */
export type QuestionOutcome =
  /** Done: the question as it is now. */
  | { readonly kind: "done"; readonly question: Question }
  /** Changed since it was read: the question as it is now, nothing written. */
  | { readonly kind: "conflict"; readonly question: Question }
  /** Not allowed; nothing written. */
  | { readonly kind: "refused"; readonly code: string; readonly text: string };

async function settle(
  api: BoardApi,
  call: Parameters<BoardApi["board"]>[0],
  id: string,
): Promise<QuestionOutcome> {
  const result = await boardCall(api, call, "questioned");
  const outcome = result.outcome;
  if (outcome.case === "conflict") {
    return { kind: "conflict", question: outcome.value };
  }
  if (outcome.case === "refused") {
    return { kind: "refused", code: outcome.value.code, text: outcome.value.text };
  }
  const question = outcome.value?.questions.find((q) => q.id === id);
  if (question === undefined) {
    return { kind: "refused", code: "question.missing", text: "The answer came back without it." };
  }
  return { kind: "done", question };
}

/** Answers `question` as read: an option, text or both. */
export function answerQuestion(
  api: BoardApi,
  question: Question,
  answer: { readonly option?: string; readonly text: string },
): Promise<QuestionOutcome> {
  return settle(
    api,
    {
      case: "questionAnswer",
      value: {
        id: question.id,
        expectedVersion: question.version,
        option: answer.option,
        text: answer.text,
      },
    },
    question.id,
  );
}

/** Dismisses `question` as read, with the owner's reason. */
export function dismissQuestion(
  api: BoardApi,
  question: Question,
  reason: string,
): Promise<QuestionOutcome> {
  return settle(
    api,
    {
      case: "questionDismiss",
      value: { id: question.id, expectedVersion: question.version, reason },
    },
    question.id,
  );
}

/** What a list asks for: a card's, a node's, open or answered. */
export interface QuestionFilter {
  readonly projectId: string;
  readonly itemId?: string;
  readonly planNodeId?: string;
  readonly state?: QuestionState;
}

/** One page of a project's questions, open ones first; `next` while there are more. */
export async function listQuestions(
  api: BoardApi,
  filter: QuestionFilter,
  before?: string,
  limit = 50,
): Promise<{ readonly questions: readonly Question[]; readonly next?: string }> {
  const page = await boardCall(
    api,
    {
      case: "questionList",
      value: {
        projectId: filter.projectId,
        itemId: filter.itemId,
        planNodeId: filter.planNodeId,
        state: filter.state,
        before,
        limit,
      },
    },
    "questions",
  );
  return { questions: page.questions, next: page.next };
}

/** Whether `q` belongs in a list asked for with `filter`. */
export function matches(q: Question, filter: QuestionFilter): boolean {
  return (
    q.projectId === filter.projectId &&
    (filter.itemId === undefined || q.itemId === filter.itemId) &&
    (filter.planNodeId === undefined || q.planNodeId === filter.planNodeId) &&
    (filter.state === undefined || q.state === filter.state)
  );
}

export function isOpen(q: Question): boolean {
  return q.state === QuestionState.OPEN;
}

/** Open ones first; otherwise as they came. */
export function openFirst(questions: readonly Question[]): Question[] {
  return [...questions.filter(isOpen), ...questions.filter((q) => !isOpen(q))];
}

/**
 * The list with `q` in it: a newer version replaces the old, a new one joins
 * at the top when `add` says it belongs. An older version changes nothing.
 */
export function withQuestion(
  list: readonly Question[],
  q: Question,
  add: boolean,
): readonly Question[] {
  const at = list.findIndex((x) => x.id === q.id);
  if (at < 0) {
    return add ? [q, ...list] : list;
  }
  const old = list[at];
  if (old !== undefined && old.version > q.version) {
    return list;
  }
  return list.map((x, i) => (i === at ? q : x));
}

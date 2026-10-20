// The words for a question (0.22.0): its state, its answer, who asked and
// what it is about.

import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { QuestionState } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { when } from "../board/drawer/drawerText";

/** "The ~250 celebrated saints": an option's label by its key, the key without one. */
export function optionLabel(q: Question, key: string | undefined): string {
  if (key === undefined || key === "") {
    return "";
  }
  return q.options.find((o) => o.key === key)?.label ?? key;
}

/** "Lead asked · 18 Oct, 08:20 · on ASL-4". */
export function askedLine(q: Question, where: boolean): string {
  const parts = [`${q.askedBy?.name ?? "A bot"} asked`, when(q.askedAt)];
  if (where && q.itemId !== undefined) {
    parts.push(`on ${q.itemId}`);
  } else if (where && q.planNodeId !== undefined) {
    parts.push(`on ${q.planNodeId}`);
  }
  return parts.filter(Boolean).join(" · ");
}

/** "You answered · 18 Oct, 09:02", or relayed by a bot. */
export function answeredLine(q: Question): string {
  const a = q.answer;
  if (a === undefined) {
    return "";
  }
  const by = a.answeredBy.startsWith("owner-via-bot:") ? "Relayed for you" : "You answered";
  return [by, when(a.answeredAt)].filter(Boolean).join(" · ");
}

/** "2 open · 1 holds the gate": a list's count, nothing when none is open. */
export function openCount(questions: readonly Question[]): string {
  const open = questions.filter((q) => q.state === QuestionState.OPEN);
  if (open.length === 0) {
    return "";
  }
  const blocking = open.filter((q) => q.blocking).length;
  return `${open.length} open${blocking > 0 ? ` · ${blocking} ${blocking === 1 ? "holds" : "hold"} the gate` : ""}`;
}

/** What a refusal means, in words. */
export function refusalText(code: string, text: string): string {
  switch (code) {
    case "question.answered":
      return "It was answered already.";
    case "question.no_option":
      return "That option isn't one of the question's.";
    case "question.empty":
      return "Pick an option or write an answer.";
    default:
      return text === "" ? code : text;
  }
}

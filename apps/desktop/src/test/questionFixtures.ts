// Questions for the owner (0.22.0), for tests and stories: the three a lead
// asked on the sources and licensing spike ASL-4 of plan node PL-2. Shapes
// follow crates/bus/fixtures/board/messages/*question*.

import { create } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { timestampFromDate } from "@bufbuild/protobuf/wkt";
import type { Question } from "../protocol/gen/flotilla/board/v1/questions_pb";
import {
  OwnerAnswerSchema,
  QuestionAskerSchema,
  QuestionPageSchema,
  QuestionResultSchema,
  QuestionSchema,
  QuestionState,
} from "../protocol/gen/flotilla/board/v1/questions_pb";
import type { AttentionRowJson } from "../protocol/dashboard";
import { QUESTIONS_V1 } from "../protocol/questions";
import { matches } from "../protocol/questions";
import { FakeDaemon } from "./fakeDaemon";
import { ItemType } from "../protocol/gen/flotilla/board/v1/board_pb";
import { PlanNodeDetailSchema } from "../protocol/gen/flotilla/board/v1/plan_pb";
import { card } from "./boardFixtures";
import { planClient, planNodes, planTree } from "./planFixtures";

const at = (iso: string) => timestampFromDate(new Date(iso));

const LEAD = create(QuestionAskerSchema, { botId: "b-lead", name: "Lead", avatar: "icon:orbit" });

function question(over: MessageInitShape<typeof QuestionSchema>): Question {
  return create(QuestionSchema, {
    projectId: "p1",
    askedBy: LEAD,
    askedAt: at("2026-10-18T08:20:51Z"),
    state: QuestionState.OPEN,
    version: 1n,
    ...over,
  });
}

/** Open and blocking, with options and a recommendation: the contract's d-7f3a. */
export function saintsQuestion(over: MessageInitShape<typeof QuestionSchema> = {}): Question {
  return question({
    id: "d-7f3a",
    text: "Should v1 cover only the ~250 celebrated saints, or the full Martyrology?",
    why: "It sets the dataset's size and the licensing work.",
    options: [
      {
        key: "celebrated",
        label: "The ~250 celebrated saints",
        description: "A smaller dataset we can check by hand.",
      },
      { key: "full", label: "The full Martyrology (~7,000)" },
    ],
    recommendation: "celebrated",
    blocking: true,
    itemId: "ASL-4",
    planNodeId: "PL-2",
    commentId: "c-91",
    ...over,
  });
}

/** Open, answered in words. */
export function calendarQuestion(): Question {
  return question({
    id: "d-7f3b",
    text: "Italy plus the universal calendar only, or every national calendar?",
    itemId: "ASL-4",
    planNodeId: "PL-2",
    commentId: "c-91",
  });
}

/** Answered with an option and a note, written into the research document. */
export function licensingQuestion(): Question {
  return question({
    id: "d-7f3c",
    text: "May we contact the LEV and the CEI about licensing?",
    options: [
      { key: "yes", label: "Yes, contact both" },
      { key: "no", label: "Not yet" },
    ],
    recommendation: "yes",
    state: QuestionState.ANSWERED,
    itemId: "ASL-4",
    planNodeId: "PL-2",
    version: 2n,
    answer: {
      option: "yes",
      text: "Start with the LEV; I'll write to the CEI myself.",
      answeredBy: "owner",
      answeredAt: at("2026-10-18T09:02:00Z"),
    },
    writtenTo: "ASL-4-sources-licensing-research.md",
  });
}

/** Dismissed with the owner's reason. */
export function fontQuestion(): Question {
  return question({
    id: "d-7f3d",
    text: "Which font should the saint cards use?",
    state: QuestionState.DISMISSED,
    itemId: "ASL-7",
    version: 2n,
    dismissedReason: "The design spike decides it.",
  });
}

function aslQuestions(): Question[] {
  return [saintsQuestion(), calendarQuestion(), licensingQuestion()];
}

/** `q` as the owner's answer leaves it. */
export function answered(q: Question, option: string | undefined, text: string): Question {
  return {
    ...q,
    state: QuestionState.ANSWERED,
    version: q.version + 1n,
    answer: create(OwnerAnswerSchema, {
      option,
      text,
      answeredBy: "owner",
      answeredAt: at("2026-10-18T09:30:00Z"),
    }),
  };
}

/**
 * A Flotilla service that serves `questions_v1`: the list, filtered as asked,
 * and answers and dismissals that settle the question.
 */
export function questionsClient(
  all: readonly Question[] = aslQuestions(),
  /** Another fake service to add them to: the plan's. */
  base: FakeDaemon = new FakeDaemon(),
): FakeDaemon {
  let questions = [...all];
  const settle = (next: Question) => {
    questions = questions.map((q) => (q.id === next.id ? next : q));
    return {
      case: "questioned" as const,
      value: create(QuestionResultSchema, {
        outcome: { case: "done", value: { questions: [next] } },
      }),
    };
  };
  const find = (id: string): Question => {
    const found = questions.find((q) => q.id === id);
    if (found === undefined) {
      throw new Error(`no question ${id}`);
    }
    return found;
  };
  const fake = base
    .onBoard("questionList", (call) => {
      const v = call.case === "questionList" ? call.value : {};
      const filter = {
        projectId: v.projectId ?? "",
        itemId: v.itemId,
        planNodeId: v.planNodeId,
        state: v.state,
      };
      return {
        case: "questions",
        value: create(QuestionPageSchema, {
          questions: questions.filter((q) => matches(q, filter)),
        }),
      };
    })
    .onBoard("questionAnswer", (call) => {
      const v = call.case === "questionAnswer" ? call.value : {};
      return settle(answered(find(v.id ?? ""), v.option, v.text ?? ""));
    })
    .onBoard("questionDismiss", (call) => {
      const v = call.case === "questionDismiss" ? call.value : {};
      const q = find(v.id ?? "");
      return settle({
        ...q,
        state: QuestionState.DISMISSED,
        version: q.version + 1n,
        dismissedReason: v.reason,
      });
    });
  fake.grants = ["read", "control", "approve"];
  fake.capabilities = [...new Set([...fake.capabilities, QUESTIONS_V1])];
  return fake;
}

/** The rows a 0.22 daemon raises for questions asked with `ask_owner`. */
export const QUESTION_ROWS: readonly AttentionRowJson[] = [
  {
    kind: "question",
    id: "question:d-mac:d-7f3b",
    title: "Italy plus the universal calendar only, or every national calendar?",
    decision_id: "d-7f3b",
    item_id: "ASL-4",
    bot: { bot_id: "b-lead", name: "Lead" },
  },
  {
    kind: "question",
    id: "question:d-mac:d-7f3a",
    title: "Should v1 cover only the ~250 celebrated saints, or the full Martyrology?",
    decision_id: "d-7f3a",
    item_id: "ASL-4",
    question_blocking: true,
    bot: { bot_id: "b-lead", name: "Lead" },
  },
];

/** Combat onboarding with three questions: one holding its gate, one on its spike, one answered. */
export function questionsPlan() {
  const nodes = planNodes();
  const onboarding = nodes.find((n) => n.id === "PL-9");
  if (onboarding !== undefined) {
    onboarding.questionsOpen = 2;
    onboarding.questionsBlocking = 1;
  }
  const questions = [
    saintsQuestion({ itemId: undefined, planNodeId: "PL-9" }),
    { ...calendarQuestion(), itemId: "CO-1", planNodeId: "PL-9" },
    { ...licensingQuestion(), itemId: "CO-1", planNodeId: "PL-9" },
  ];
  const fake = questionsClient(questions, planClient(planTree({ nodes })));
  fake.onBoard("planNodeGet", (call) => {
    const id = call.case === "planNodeGet" ? call.value.id : "";
    return {
      case: "planNode",
      value: create(PlanNodeDetailSchema, {
        node: nodes.find((n) => n.id === id),
        cards:
          id === "PL-9"
            ? [card({ id: "CO-1", title: "What players miss", type: ItemType.SPIKE })]
            : [],
        questions: id === "PL-9" ? questions : [],
      }),
    };
  });
  return fake;
}

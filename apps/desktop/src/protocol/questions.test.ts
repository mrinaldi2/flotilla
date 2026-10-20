import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fromJson, toBinary } from "@bufbuild/protobuf";
import type { JsonValue } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import { FakeDaemon } from "../test/fakeDaemon";
import { calendarQuestion, licensingQuestion, saintsQuestion } from "../test/questionFixtures";
import { BinaryChannel } from "./channel";
import { QuestionState } from "./gen/flotilla/board/v1/questions_pb";
import { EnvelopeSchema } from "./gen/flotilla/wire/v1/envelope_pb";
import type { BoardResponse } from "./gen/flotilla/board/v1/requests_pb";
import { BoardResponseSchema } from "./gen/flotilla/board/v1/requests_pb";
import {
  answerQuestion,
  dismissQuestion,
  listQuestions,
  matches,
  openFirst,
  QUESTIONS_V1,
  questionsOn,
  withQuestion,
} from "./questions";

const FIXTURES = join(__dirname, "../../../../crates/bus/fixtures/board/messages");

/** A contract fixture's board response. */
function response(name: string): BoardResponse["response"] {
  const raw = JSON.parse(readFileSync(join(FIXTURES, name), "utf8")) as {
    readonly boardResponse: JsonValue;
  };
  return fromJson(BoardResponseSchema, raw.boardResponse).response;
}

function answering(name: string): FakeDaemon {
  const reply = response(name);
  return new FakeDaemon()
    .onBoard("questionAnswer", () => reply)
    .onBoard("questionDismiss", () => reply)
    .onBoard("questionList", () => reply);
}

describe("questions over the board", () => {
  it("is on only with questions_v1", () => {
    const fake = new FakeDaemon();
    expect(questionsOn(fake)).toBe(false);
    fake.capabilities = [QUESTIONS_V1];
    expect(questionsOn(fake)).toBe(true);
  });

  it("answers with the version read, and takes the question done", async () => {
    const fake = answering("response.questioned.done.json");
    const outcome = await answerQuestion(fake, saintsQuestion(), {
      option: "celebrated",
      text: "",
    });
    expect(fake.boardCalls[0]).toEqual({
      case: "questionAnswer",
      value: { id: "d-7f3a", expectedVersion: 1n, option: "celebrated", text: "" },
    });
    expect(outcome.kind).toBe("done");
    expect(outcome.kind === "done" ? outcome.question.state : undefined).toBe(
      QuestionState.ANSWERED,
    );
  });

  it("hands back the question as it is now on a conflict", async () => {
    const outcome = await answerQuestion(
      answering("response.questioned.conflict.json"),
      saintsQuestion(),
      { text: "All of them" },
    );
    expect(outcome.kind).toBe("conflict");
    expect(outcome.kind === "conflict" ? outcome.question.answer?.option : "").toBe("celebrated");
  });

  it("says why a dismissal was refused", async () => {
    const fake = answering("response.questioned.refused.json");
    const outcome = await dismissQuestion(fake, saintsQuestion(), "Not needed");
    expect(fake.boardCalls[0]).toEqual({
      case: "questionDismiss",
      value: { id: "d-7f3a", expectedVersion: 1n, reason: "Not needed" },
    });
    expect(outcome).toEqual({
      kind: "refused",
      code: "question.answered",
      text: "This question was already answered.",
    });
  });

  it("refuses a done answer that leaves the question out", async () => {
    const outcome = await answerQuestion(
      answering("response.questioned.done.json"),
      calendarQuestion(),
      { text: "Italy" },
    );
    expect(outcome.kind).toBe("refused");
  });

  it("reads a page and its cursor", async () => {
    const fake = answering("response.questions.json");
    const page = await listQuestions(fake, { projectId: "p1", itemId: "ASL-4" }, "c1", 20);
    expect(fake.boardCalls[0]).toEqual({
      case: "questionList",
      value: {
        projectId: "p1",
        itemId: "ASL-4",
        planNodeId: undefined,
        state: undefined,
        before: "c1",
        limit: 20,
      },
    });
    expect(page.questions.map((q) => q.id)).toEqual(["d-7f3a"]);
    expect(page.next).toBe("2026-10-18T08:20:51Z|d-7f3a");
  });
});

describe("question lists", () => {
  it("matches a filter by project, card, node and state", () => {
    const q = saintsQuestion();
    expect(matches(q, { projectId: "p1" })).toBe(true);
    expect(matches(q, { projectId: "p2" })).toBe(false);
    expect(matches(q, { projectId: "p1", itemId: "ASL-7" })).toBe(false);
    expect(matches(q, { projectId: "p1", planNodeId: "PL-2" })).toBe(true);
    expect(matches(q, { projectId: "p1", state: QuestionState.ANSWERED })).toBe(false);
  });

  it("puts open ones first", () => {
    const list = openFirst([licensingQuestion(), saintsQuestion()]);
    expect(list.map((q) => q.id)).toEqual(["d-7f3a", "d-7f3c"]);
  });

  it("replaces a question by a newer version, never an older one, and adds new ones", () => {
    const old = saintsQuestion();
    const newer = saintsQuestion({ version: 3n, text: "Newer" });
    expect(withQuestion([newer], old, true)[0]?.text).toBe("Newer");
    expect(withQuestion([old], newer, true)[0]?.text).toBe("Newer");
    expect(withQuestion([old], calendarQuestion(), false)).toHaveLength(1);
    expect(withQuestion([old], calendarQuestion(), true).map((q) => q.id)).toEqual([
      "d-7f3b",
      "d-7f3a",
    ]);
  });

  it("hands the contract's push to the question subscribers", () => {
    const raw = JSON.parse(
      readFileSync(join(FIXTURES, "push.question_change.json"), "utf8"),
    ) as JsonValue;
    const channel = new BinaryChannel();
    const seen: string[] = [];
    const off = channel.onQuestion((change) => seen.push(change.question?.id ?? ""));
    channel.receive(toBinary(EnvelopeSchema, fromJson(EnvelopeSchema, raw)));
    off();
    channel.receive(toBinary(EnvelopeSchema, fromJson(EnvelopeSchema, raw)));
    expect(seen).toEqual(["d-7f3a"]);
  });
});

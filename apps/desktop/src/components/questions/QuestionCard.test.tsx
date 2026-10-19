import { create } from "@bufbuild/protobuf";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Question } from "../../protocol/gen/flotilla/board/v1/questions_pb";
import {
  QuestionResultSchema,
  QuestionState,
} from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { DaemonError } from "../../protocol/connection";
import {
  answered,
  calendarQuestion,
  fontQuestion,
  licensingQuestion,
  questionsClient,
  saintsQuestion,
} from "../../test/questionFixtures";
import QuestionCard from "./QuestionCard";

function setup(question: Question, canAnswer = true, fake = questionsClient()) {
  const onChanged = vi.fn<(q: Question) => void>();
  render(
    <ul>
      <QuestionCard api={fake} question={question} canAnswer={canAnswer} onChanged={onChanged} />
    </ul>,
  );
  return { fake, onChanged };
}

describe("QuestionCard", () => {
  it("shows the question, why, who asked, the recommended option and the gate it holds", () => {
    setup(saintsQuestion());
    const card = screen.getByRole("listitem", { name: /celebrated saints/ });
    expect(card).toHaveTextContent("It sets the dataset's size and the licensing work.");
    expect(card).toHaveTextContent("Lead asked");
    expect(card).toHaveTextContent("Holds the gate");
    expect(screen.getByRole("button", { name: /The ~250 celebrated saints/ })).toHaveTextContent(
      "Recommended",
    );
  });

  it("answers with the option picked and a note", async () => {
    const user = userEvent.setup();
    const { fake, onChanged } = setup(saintsQuestion());
    const answer = screen.getByRole("button", { name: "Answer" });
    expect(answer).toBeDisabled();
    await user.click(screen.getByRole("button", { name: /The full Martyrology/ }));
    expect(screen.getByRole("button", { name: /The full Martyrology/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await user.type(screen.getByRole("textbox", { name: /Your answer/ }), "  All of them ");
    await user.click(answer);
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
    expect(fake.boardCalls).toContainEqual({
      case: "questionAnswer",
      value: { id: "d-7f3a", expectedVersion: 1n, option: "full", text: "All of them" },
    });
    expect(onChanged.mock.calls[0]?.[0].state).toBe(QuestionState.ANSWERED);
  });

  it("unpicks an option clicked twice, and answers a question without options in words", async () => {
    const user = userEvent.setup();
    const { fake } = setup(calendarQuestion());
    expect(screen.queryByRole("group", { name: "Options" })).toBeNull();
    await user.type(screen.getByRole("textbox", { name: /Your answer/ }), "Italy only");
    await user.click(screen.getByRole("button", { name: "Answer" }));
    await waitFor(() =>
      expect(fake.boardCalls).toContainEqual({
        case: "questionAnswer",
        value: { id: "d-7f3b", expectedVersion: 1n, option: undefined, text: "Italy only" },
      }),
    );
  });

  it("dismisses with a reason, and cancels without one", async () => {
    const user = userEvent.setup();
    const { fake, onChanged } = setup(saintsQuestion());
    await user.click(screen.getByRole("button", { name: "Dismiss…" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await user.click(screen.getByRole("button", { name: "Dismiss…" }));
    const dismiss = screen.getByRole("button", { name: "Dismiss" });
    expect(dismiss).toBeDisabled();
    await user.type(screen.getByRole("textbox", { name: "Why dismiss it" }), "Asked twice");
    await user.click(dismiss);
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
    expect(fake.boardCalls).toContainEqual({
      case: "questionDismiss",
      value: { id: "d-7f3a", expectedVersion: 1n, reason: "Asked twice" },
    });
  });

  it("takes the question as it is now when it changed meanwhile", async () => {
    const user = userEvent.setup();
    const now = answered(saintsQuestion(), "celebrated", "Start small");
    const fake = questionsClient().onBoard("questionAnswer", () => ({
      case: "questioned",
      value: create(QuestionResultSchema, { outcome: { case: "conflict", value: now } }),
    }));
    const { onChanged } = setup(saintsQuestion(), true, fake);
    await user.click(screen.getByRole("button", { name: /celebrated saints/ }));
    await user.click(screen.getByRole("button", { name: "Answer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("It was settled meanwhile");
    expect(onChanged).toHaveBeenCalledWith(now);
  });

  it("asks to answer again when it changed but is still open", async () => {
    const user = userEvent.setup();
    const now = saintsQuestion({ version: 2n });
    const fake = questionsClient().onBoard("questionAnswer", () => ({
      case: "questioned",
      value: create(QuestionResultSchema, { outcome: { case: "conflict", value: now } }),
    }));
    setup(saintsQuestion(), true, fake);
    await user.click(screen.getByRole("button", { name: /celebrated saints/ }));
    await user.click(screen.getByRole("button", { name: "Answer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("answer again");
  });

  it("says why an answer was refused, and what failed", async () => {
    const user = userEvent.setup();
    const fake = questionsClient().onBoard("questionAnswer", () => ({
      case: "questioned",
      value: create(QuestionResultSchema, {
        outcome: { case: "refused", value: { code: "question.no_option", text: "" } },
      }),
    }));
    const { onChanged } = setup(saintsQuestion(), true, fake);
    await user.click(screen.getByRole("button", { name: /celebrated saints/ }));
    await user.click(screen.getByRole("button", { name: "Answer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("isn't one of the question's");
    expect(onChanged).not.toHaveBeenCalled();
    fake.onBoard("questionAnswer", () => {
      throw new DaemonError("forbidden", "needs the approve grant");
    });
    await user.click(screen.getByRole("button", { name: "Answer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("needs the approve grant");
  });

  it("only shows the options to a connection that can't answer", () => {
    setup(saintsQuestion(), false);
    expect(screen.getByRole("button", { name: /celebrated saints/ })).toBeDisabled();
    expect(screen.queryByRole("textbox")).toBeNull();
    expect(screen.queryByRole("button", { name: "Answer" })).toBeNull();
  });

  it("shows an answer with its option, note and where it was written", () => {
    setup(licensingQuestion());
    const card = screen.getByRole("listitem");
    expect(card).toHaveTextContent("Yes, contact both · Start with the LEV");
    expect(card).toHaveTextContent("You answered");
    expect(card).toHaveTextContent("added to ASL-4-sources-licensing-research.md");
    expect(screen.queryByRole("button", { name: "Answer" })).toBeNull();
  });

  it("shows an answer a bot relayed", () => {
    const relayed = licensingQuestion();
    const q = {
      ...relayed,
      answer: relayed.answer && { ...relayed.answer, answeredBy: "owner-via-bot:b1" },
    };
    setup(q);
    expect(screen.getByRole("listitem")).toHaveTextContent("Relayed for you");
  });

  it("shows a dismissal's reason", () => {
    setup(fontQuestion());
    expect(screen.getByRole("listitem")).toHaveTextContent(
      "Dismissed: The design spike decides it.",
    );
  });
});

import { create } from "@bufbuild/protobuf";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import {
  QuestionPageSchema,
  QuestionState,
} from "../../protocol/gen/flotilla/board/v1/questions_pb";
import { node } from "../../test/planFixtures";
import {
  calendarQuestion,
  fontQuestion,
  licensingQuestion,
  questionsClient,
  saintsQuestion,
} from "../../test/questionFixtures";
import QuestionsView from "./QuestionsView";

function setup(
  fake = questionsClient([
    saintsQuestion(),
    calendarQuestion(),
    licensingQuestion(),
    fontQuestion(),
  ]),
) {
  fake
    .onBoard("boardWatch", () => {
      throw new Error("no board");
    })
    .onBoard("boardUnwatch", () => {
      throw new Error("no board");
    });
  const onOpenItem = vi.fn<(id: string) => void>();
  render(
    <QuestionsView
      api={fake}
      projectId="p1"
      connected
      canAnswer
      nodes={[node({ id: "PL-2", title: "Saints dataset" })]}
      onOpenItem={onOpenItem}
    />,
  );
  return { fake, onOpenItem };
}

describe("QuestionsView", () => {
  it("lists the open questions, each with its card, and watches the project", async () => {
    const { fake } = setup();
    expect(await screen.findByText(/cover only the/)).toBeInTheDocument();
    const list = screen.getAllByRole("listitem");
    expect(list).toHaveLength(2);
    expect(list[0]).toHaveTextContent("on ASL-4");
    expect(fake.boardCalls[0]).toEqual({ case: "boardWatch", value: { projectId: "p1" } });
  });

  it("shows the answered ones, then all, and says when there are none", async () => {
    const user = userEvent.setup();
    setup();
    await screen.findByText(/cover only the/);
    await user.click(screen.getByRole("button", { name: "Answered" }));
    expect(await screen.findByText(/LEV and the CEI/)).toBeInTheDocument();
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: "All" }));
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(4));
    await user.selectOptions(screen.getByRole("combobox", { name: "About" }), "item:ASL-7");
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(1));
    await user.click(screen.getByRole("button", { name: "Open" }));
    expect(await screen.findByText("No question waits for you.")).toBeInTheDocument();
  });

  it("filters by plan node, and opens a card picked", async () => {
    const user = userEvent.setup();
    const { fake, onOpenItem } = setup();
    await screen.findByText(/cover only the/);
    await user.selectOptions(screen.getByRole("combobox", { name: "About" }), "node:PL-2");
    await waitFor(() =>
      expect(fake.boardCalls.at(-1)).toMatchObject({
        case: "questionList",
        value: { planNodeId: "PL-2", state: QuestionState.OPEN },
      }),
    );
    await user.selectOptions(screen.getByRole("combobox", { name: "About" }), "item:ASL-4");
    await user.click(screen.getByRole("button", { name: "Open ASL-4 on the board" }));
    expect(onOpenItem).toHaveBeenCalledWith("ASL-4");
  });

  it("reads older pages from the cursor", async () => {
    const user = userEvent.setup();
    const fake = questionsClient().onBoard("questionList", (call) => {
      const before = call.case === "questionList" ? call.value.before : undefined;
      return {
        case: "questions",
        value: create(QuestionPageSchema, {
          questions: [before === undefined ? saintsQuestion() : calendarQuestion()],
          next: before === undefined ? "c1" : undefined,
        }),
      };
    });
    setup(fake);
    await user.click(await screen.findByRole("button", { name: "Show older" }));
    expect(await screen.findByText(/national calendar/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Show older" })).toBeNull();
    expect(fake.boardCalls.at(-1)).toMatchObject({ value: { before: "c1" } });
  });

  it("follows the project's pushes and says what failed", async () => {
    const fake = questionsClient([]);
    setup(fake);
    expect(await screen.findByText("No question waits for you.")).toBeInTheDocument();
    fake.emitQuestionChange({
      $typeName: "flotilla.board.v1.QuestionChange",
      projectId: "p1",
      question: calendarQuestion(),
    });
    expect(await screen.findByText(/national calendar/)).toBeInTheDocument();
    const list = screen.getByRole("list");
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);
  });

  it("says why the list couldn't be read", async () => {
    const fake = questionsClient().onBoard("questionList", () => {
      throw new Error("unsupported");
    });
    setup(fake);
    expect(await screen.findByText("unsupported")).toBeInTheDocument();
  });
});

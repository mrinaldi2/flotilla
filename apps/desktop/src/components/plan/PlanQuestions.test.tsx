import { create } from "@bufbuild/protobuf";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { PlanNodeDetailSchema } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { planNodes } from "../../test/planFixtures";
import { PlanWindow } from "../../test/planStory";
import { questionsPlan, saintsQuestion } from "../../test/questionFixtures";

describe("questions in the Plan tab", () => {
  it("counts a node's open questions in the tree", async () => {
    render(<PlanWindow client={questionsPlan()} />);
    const tree = await screen.findByRole("complementary", { name: "Plan tree" });
    expect(
      within(tree).getByRole("button", {
        name: "Initiative: Combat onboarding, Research, 1 spike, 2 questions for you, 1 holds the gate",
      }),
    ).toHaveTextContent("?2");
  });

  it("lists the open ones under Research, the answered folded, and answers one", async () => {
    const user = userEvent.setup();
    const fake = questionsPlan();
    render(<PlanWindow client={fake} openNode="PL-9" />);
    const research = await screen.findByRole("region", { name: "Research" });
    const questions = await within(research).findByLabelText("Questions");
    expect(within(questions).getByRole("heading")).toHaveTextContent(
      "Questions · 2 open · 1 holds the gate",
    );
    expect(within(questions).getByText("Holds the gate")).toBeVisible();
    expect(within(questions).getByText("1 answered")).toBeVisible();
    expect(
      within(questions).getByRole("listitem", { name: /national calendar/ }),
    ).toHaveTextContent("on CO-1");
    await user.type(
      within(questions).getByRole("textbox", { name: /national calendar/ }),
      "Italy only",
    );
    const answers = within(questions).getAllByRole("button", { name: "Answer" });
    await user.click(answers[1] ?? answers[0]);
    await waitFor(() =>
      expect(within(questions).getByRole("heading")).toHaveTextContent("Questions · 1 open"),
    );
    expect(within(questions).getByText("2 answered")).toBeVisible();
  });

  it("shows a node's questions in their own section when it has no research", async () => {
    const fake = questionsPlan();
    fake.onBoard("planNodeGet", () => ({
      case: "planNode",
      value: create(PlanNodeDetailSchema, {
        node: planNodes().find((n) => n.id === "PL-3"),
        questions: [saintsQuestion({ planNodeId: "PL-3", itemId: undefined })],
      }),
    }));
    render(<PlanWindow client={fake} openNode="PL-3" />);
    const section = await screen.findByRole("region", { name: "Questions for you" });
    expect(section).toHaveTextContent("cover only the");
  });
});

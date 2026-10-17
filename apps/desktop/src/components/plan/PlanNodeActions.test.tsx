import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import {
  PlanKind,
  PlanNodeSchema,
  PlanResultSchema,
  PlanStage,
} from "../../protocol/gen/flotilla/board/v1/plan_pb";
import type { FakeDaemon } from "../../test/fakeDaemon";
import { hudEpic, planClient, planned, planNodes } from "../../test/planFixtures";
import { requestsTree } from "../../test/planRequests";
import { PlanWindow } from "../../test/planStory";
import { formOf, goalMove, updateOf } from "./PlanNodeActions";

function calls<C extends string>(fake: FakeDaemon, arm: C) {
  return fake.boardCalls.flatMap((c) => (c.case === arm ? [c.value] : []));
}

const goal = (stage: PlanStage) => create(PlanNodeSchema, { kind: PlanKind.GOAL, stage });

describe("a node's own actions", () => {
  it("moves a goal from define to active, and from active to met, only", () => {
    expect(goalMove(goal(PlanStage.DEFINE))).toEqual({
      stage: PlanStage.ACTIVE,
      label: "Make active",
    });
    expect(goalMove(goal(PlanStage.ACTIVE))).toEqual({ stage: PlanStage.MET, label: "Mark met" });
    expect(goalMove(goal(PlanStage.MET))).toBeNull();
    expect(goalMove(hudEpic())).toBeNull();
  });

  it("sends only what changed; an empty deadline or lead clears it", () => {
    const epic = hudEpic();
    expect(updateOf(epic, formOf(epic))).toBeNull();
    expect(updateOf(epic, { ...formOf(epic), title: " HUD clarity ", lead: "" })).toEqual({
      id: "PL-8",
      expectedVersion: 6n,
      title: "HUD clarity",
      lead: "",
    });
    const [first] = planNodes();
    if (first === undefined) {
      throw new Error("the fixture has a goal");
    }
    expect(updateOf(first, { ...formOf(first), due: "", lead: "b1" })).toEqual({
      id: "PL-1",
      expectedVersion: 3n,
      due: "",
    });
  });

  it("marks an active goal met", async () => {
    const user = userEvent.setup();
    const fake = planClient().onBoard("planUpdate", () => ({
      case: "planned",
      value: planned(goal(PlanStage.MET)),
    }));
    render(<PlanWindow client={fake} openNode="PL-1" />);
    await user.click(await screen.findByRole("button", { name: "Mark met" }));
    expect(calls(fake, "planUpdate")).toMatchObject([
      { id: "PL-1", expectedVersion: 3n, stage: PlanStage.MET },
    ]);
  });

  it("edits a node, and says when it changed meanwhile", async () => {
    const user = userEvent.setup();
    const fake = planClient().onBoard("planUpdate", () => ({
      case: "planned",
      value: create(PlanResultSchema, {
        outcome: { case: "conflict", value: { ...hudEpic(), version: 7n } },
      }),
    }));
    render(<PlanWindow client={fake} openNode="PL-8" />);
    await user.click(await screen.findByRole("button", { name: "Edit" }));
    const form = screen.getByRole("form", { name: "Edit Combat HUD clarity" });
    await user.clear(within(form).getByRole("textbox", { name: "Title" }));
    await user.type(within(form).getByRole("textbox", { name: "Title" }), "HUD at a glance");
    await user.selectOptions(within(form).getByRole("combobox", { name: "Lead" }), "b2");
    await user.click(within(form).getByRole("button", { name: "Save" }));
    expect(calls(fake, "planUpdate")).toMatchObject([
      { id: "PL-8", expectedVersion: 6n, title: "HUD at a glance", lead: "b2" },
    ]);
    expect(await within(form).findByRole("alert")).toHaveTextContent("changed meanwhile");
  });

  it("drops a node with a reason, after saying what stops", async () => {
    const user = userEvent.setup();
    const fake = planClient().onBoard("planDrop", () => ({
      case: "planned",
      value: planned({ ...hudEpic(), stage: PlanStage.DROPPED, gate: undefined }),
    }));
    render(<PlanWindow client={fake} openNode="PL-8" />);
    await user.click(await screen.findByRole("button", { name: "Drop…" }));
    const dialog = screen.getByRole("dialog", { name: "Drop Combat HUD clarity" });
    expect(dialog).toHaveTextContent("Their cards stay on the board.");
    await user.type(
      within(dialog).getByRole("textbox", { name: /Why/ }),
      "Folded into telegraphing.",
    );
    await user.click(within(dialog).getByRole("button", { name: "Drop the epic" }));
    expect(calls(fake, "planDrop")).toMatchObject([
      { id: "PL-8", reason: "Folded into telegraphing." },
    ]);
    expect(
      await screen.findByRole("button", { name: "Epic: Combat HUD clarity, Dropped" }),
    ).toBeVisible();
    expect(screen.queryByRole("button", { name: "Drop…" })).not.toBeInTheDocument();
  });

  it("offers a connection without the owner's grant only Edit", async () => {
    const fake = planClient();
    fake.grants = ["read", "control"];
    render(<PlanWindow client={fake} openNode="PL-1" />);
    expect(await screen.findByRole("button", { name: "Edit" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Mark met" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Drop…" })).not.toBeInTheDocument();
  });
});

describe("the lead's requests", () => {
  it("accepts or declines a proposed initiative, a decline with a note", async () => {
    const user = userEvent.setup();
    const fake = planClient(requestsTree()).onBoard("planRule", () => ({
      case: "planned",
      value: planned(hudEpic()),
    }));
    render(<PlanWindow client={fake} openNode="PL-12" openGate />);
    const gate = await screen.findByRole("complementary", { name: "Gate" });
    expect(
      within(gate).getByRole("heading", { name: "Accept the proposed initiative?" }),
    ).toBeVisible();
    expect(
      within(gate).getByText("Six of ten playtesters named a boss fight as the low point."),
    ).toBeVisible();
    expect(screen.getByText(/damage sponges/)).toBeVisible();
    expect(within(gate).getByRole("button", { name: "Accept" })).toBeEnabled();
    await user.type(
      within(gate).getByRole("textbox", { name: /Note to the lead/ }),
      "After the demo.",
    );
    await user.click(within(gate).getByRole("button", { name: "Decline" }));
    expect(calls(fake, "planRule")).toMatchObject([
      { id: "PL-12", approve: false, note: "After the demo." },
    ]);
  });

  it("grants or refuses a request to skip research, with the lead's reason", async () => {
    const user = userEvent.setup();
    const fake = planClient(requestsTree()).onBoard("planRule", () => ({
      case: "planned",
      value: planned(hudEpic()),
    }));
    render(<PlanWindow client={fake} openNode="PL-9" openGate />);
    const gate = await screen.findByRole("complementary", { name: "Gate" });
    expect(within(gate).getByRole("heading", { name: "Skip research?" })).toBeVisible();
    expect(
      within(gate).getByText("The tutorial study from 0.18 already answers it."),
    ).toBeVisible();
    expect(within(gate).getByRole("button", { name: "Keep research" })).toBeDisabled();
    await user.click(within(gate).getByRole("button", { name: "Skip research" }));
    expect(calls(fake, "planRule")).toMatchObject([{ id: "PL-9", approve: true, leaveOut: [] }]);
  });
});

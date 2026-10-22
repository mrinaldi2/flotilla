import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { PlanGateSchema, PlanKind, PlanStage } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import type { PlanNode } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import type { FakeDaemon } from "../../test/fakeDaemon";
import { hudEpic, node, planClient, planned, telegraphEpic } from "../../test/planFixtures";
import { awaitingClient, awaitingResearch, hldClient, pushClient } from "../../test/planRequests";
import { PlanWindow } from "../../test/planStory";
import { gateWords } from "./PlanGatePanel";
import PlanGateView from "./PlanGateView";
import { stepsOf } from "./PlanStepper";
import { nextStepWords } from "./planText";

function rulings(fake: FakeDaemon) {
  return fake.boardCalls.flatMap((c) => (c.case === "planRule" ? [c.value] : []));
}

const gate = (stage: PlanStage) => create(PlanGateSchema, { stage });

describe("research waiting for the owner", () => {
  it("asks to approve the findings, and keeps the lead's skip request", () => {
    const awaiting = awaitingResearch();
    expect(gateWords(awaiting.gate ?? gate(PlanStage.RESEARCH), awaiting)).toEqual({
      question: "Approve the research findings of Combat onboarding?",
      approve: "Approve findings",
      sendBack: "Send back",
    });
    expect(gateWords(gate(PlanStage.RESEARCH)).question).toBe("Skip research?");
    expect(stepsOf(awaiting)[0]?.sub).toBe("Waiting for you · 2 spikes");
  });

  it("shows the spikes, their documents and the questions answered", async () => {
    const user = userEvent.setup();
    const fake = awaitingClient();
    render(<PlanWindow client={fake} openNode="PL-9" openGate />);
    const panel = await screen.findByRole("complementary", { name: "Gate" });
    expect(
      within(panel).getByRole("heading", {
        name: "Approve the research findings of Combat onboarding?",
      }),
    ).toBeVisible();
    expect(within(panel).getByText(/Submitted by Flotilla · 4 Mar/)).toBeVisible();
    const spikes = within(panel).getByRole("region", { name: "Spikes" });
    expect(await within(spikes).findByText(/What players miss/)).toBeVisible();
    expect(within(spikes).getAllByText("· document attached")).toHaveLength(2);
    const answers = await within(panel).findByRole("region", { name: "Questions answered" });
    expect(within(answers).getByText("2 answered, 0 open")).toBeVisible();
    expect(within(answers).getByText("You alone, desktop and phone")).toBeVisible();
    expect(within(answers).getByText("Yes, after the first block")).toBeVisible();
    const next = within(panel).getByRole("region", { name: "Approving moves it to" });
    expect(next).toHaveTextContent("PRD: the lead writes the PRD next");
    const docs = screen.getByRole("list", { name: "Research documents" });
    expect(within(docs).getByRole("button", { name: /what-players-miss.md/ })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    await user.click(within(docs).getByRole("button", { name: /tutorial-study.md · CO-2/ }));
    expect(await screen.findByRole("heading", { name: "The 0.18 tutorial study" })).toBeVisible();
    const reads = fake.boardCalls.flatMap((c) => (c.case === "planDocRead" ? [c.value.path] : []));
    expect(reads).toContain("docs/initiatives/combat-onboarding/research/tutorial-study.md");
  });

  it("approves the findings", async () => {
    const user = userEvent.setup();
    const fake = awaitingClient().onBoard("planRule", () => ({
      case: "planned",
      value: planned(awaitingResearch()),
    }));
    render(<PlanWindow client={fake} openNode="PL-9" openGate />);
    const panel = await screen.findByRole("complementary", { name: "Gate" });
    await user.click(within(panel).getByRole("button", { name: "Approve findings" }));
    expect(rulings(fake)).toMatchObject([
      { id: "PL-9", expectedVersion: 7n, approve: true, note: undefined, leaveOut: [] },
    ]);
  });

  it("sends the findings back only with a note", async () => {
    const user = userEvent.setup();
    const fake = awaitingClient().onBoard("planRule", () => ({
      case: "planned",
      value: planned(awaitingResearch()),
    }));
    render(<PlanWindow client={fake} openNode="PL-9" openGate />);
    const panel = await screen.findByRole("complementary", { name: "Gate" });
    const back = within(panel).getByRole("button", { name: "Send back" });
    expect(back).toBeDisabled();
    await user.type(
      within(panel).getByRole("textbox", { name: /needed to send back/ }),
      "Ask a second player.",
    );
    await user.click(back);
    expect(rulings(fake)).toMatchObject([
      { id: "PL-9", approve: false, note: "Ask a second player.", leaveOut: [] },
    ]);
  });

  it("marks an open blocking question as holding the gate", async () => {
    render(<PlanWindow client={awaitingClient(true)} openNode="PL-9" openGate />);
    const answers = await screen.findByRole("region", { name: "Questions answered" });
    expect(within(answers).getByText("2 answered, 1 open")).toBeVisible();
    expect(
      within(answers).getByText("Holds the gate: Test with players who never played a soulslike?"),
    ).toBeVisible();
  });

  it("opens from the node's Review the gate", async () => {
    const user = userEvent.setup();
    render(<PlanWindow client={awaitingClient()} openNode="PL-9" />);
    const main = await screen.findByRole("main", { name: "Combat onboarding" });
    expect(within(main).getByText("Waiting for your review")).toBeVisible();
    await user.click(within(main).getByRole("button", { name: "Review the gate" }));
    expect(
      await screen.findByRole("heading", {
        name: "Approve the research findings of Combat onboarding?",
      }),
    ).toBeVisible();
  });
});

describe("the gate panel at every step", () => {
  it("says where approving moves the node", () => {
    const at = (kind: PlanKind, stage: PlanStage): string =>
      nextStepWords(node({ kind }), gate(stage));
    expect(at(PlanKind.INITIATIVE, PlanStage.PROPOSED)).toBe(
      "Research: the lead opens spikes on its board",
    );
    expect(at(PlanKind.EPIC, PlanStage.RESEARCH)).toBe("Design: the lead writes the design next");
    expect(at(PlanKind.EPIC, PlanStage.DESIGN)).toBe("HLD: the lead writes the HLD next");
    expect(at(PlanKind.INITIATIVE, PlanStage.PRD)).toBe(
      "Running: its epics start, each in research",
    );
    expect(at(PlanKind.FEATURE, PlanStage.LLD)).toBe(
      "Build: its tasks go on the initiative's board",
    );
    expect(at(PlanKind.GOAL, PlanStage.ACTIVE)).toBe("Met");
    expect(at(PlanKind.GOAL, PlanStage.PRD)).toBe("");
  });

  it("shows an HLD's questions answered, its children and the next step", async () => {
    render(<PlanWindow client={hldClient()} openNode="PL-8" openGate />);
    const panel = await screen.findByRole("complementary", { name: "Gate" });
    expect(within(panel).getByRole("group", { name: "Features it creates" })).toBeVisible();
    const answers = await within(panel).findByRole("region", { name: "Questions answered" });
    expect(within(answers).getByText("1 answered, 0 open")).toBeVisible();
    expect(
      within(answers).getByText(
        "Yes, contact both · Start with the LEV; I'll write to the CEI myself.",
      ),
    ).toBeVisible();
    expect(within(panel).getByRole("region", { name: "Approving moves it to" })).toHaveTextContent(
      "Running: its features start, each in LLD",
    );
    expect(within(panel).queryByRole("region", { name: "Spikes" })).not.toBeInTheDocument();
    expect(screen.getByRole("list", { name: "Stages" })).toBeVisible();
  });
});

describe("push requests on a gate", () => {
  it("lists the gate's push requests on a service that has them", async () => {
    render(<PlanWindow client={pushClient()} openNode="PL-8" openGate />);
    const panel = await screen.findByRole("complementary", { name: "Gate" });
    expect(await within(panel).findByRole("region", { name: "Push requests" })).toBeVisible();
  });

  it("asks an older service for none", async () => {
    const fake = hldClient();
    render(<PlanWindow client={fake} openNode="PL-8" openGate />);
    await screen.findByRole("region", { name: "Questions answered" });
    expect(screen.queryByRole("region", { name: "Push requests" })).not.toBeInTheDocument();
    expect(fake.requests.map((r) => r.body.type)).not.toContain("owner_action_list");
  });

  it("hands the slot the node's cards: spikes, documents' and its own", async () => {
    const extras = vi.fn<(node: PlanNode, itemIds: readonly string[]) => null>(() => null);
    const epic = telegraphEpic();
    render(
      <PlanGateView
        api={planClient()}
        projectId="p1"
        node={epic}
        gate={hudEpic().gate ?? gate(PlanStage.HLD)}
        ancestors={[]}
        canRule
        who={() => "Game Lead"}
        onBack={() => {}}
        onRuled={() => {}}
        extras={extras}
      />,
    );
    await screen.findByRole("region", { name: "Approving moves it to" });
    await vi.waitFor(() => {
      expect(extras).toHaveBeenLastCalledWith(epic, ["CR-12", "CR-14", "CR-15", "CR-30"]);
    });
  });
});

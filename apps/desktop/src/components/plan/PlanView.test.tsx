import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { PlanKind, PlanResultSchema } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { FakeDaemon } from "../../test/fakeDaemon";
import { hudEpic, planClient, planned, planTree } from "../../test/planFixtures";
import { PlanWindow } from "../../test/planStory";

function rulings(fake: FakeDaemon) {
  return fake.boardCalls.flatMap((c) => (c.case === "planRule" ? [c.value] : []));
}

describe("the Plan tab", () => {
  it("shows the tree with its stage chips, and filters it", async () => {
    const user = userEvent.setup();
    render(<PlanWindow />);
    const tree = await screen.findByRole("complementary", { name: "Plan tree" });
    expect(
      within(tree).getByRole("button", { name: /Combat HUD clarity.*HLD · you/ }),
    ).toBeVisible();
    expect(
      within(tree).getByRole("button", { name: /Hitstop on heavy hits.*3\/5.*Build/ }),
    ).toBeVisible();
    await user.click(within(tree).getByRole("button", { name: "Needs you 1" }));
    expect(within(tree).getAllByRole("listitem")).toHaveLength(3);
    await user.click(within(tree).getByRole("button", { name: "Done" }));
    expect(within(tree).getByText("Nothing is done yet.")).toBeVisible();
  });

  it("opens a node with its stages, research spikes, documents and history", async () => {
    const user = userEvent.setup();
    const opened = vi.fn<(id: string) => void>();
    render(<PlanWindow onOpenItem={opened} />);
    await user.click(await screen.findByRole("button", { name: /Enemy attack telegraphing/ }));
    const main = await screen.findByRole("main", { name: "Enemy attack telegraphing" });
    const stages = within(main).getByRole("list", { name: "Stages" });
    expect(within(stages).getByText("Closed · 3 spikes")).toBeVisible();
    expect(within(stages).getByRole("listitem", { current: "step" })).toHaveTextContent("Design");
    await user.click(
      await within(main).findByRole("button", { name: "Prototype three telegraph styles" }),
    );
    expect(opened).toHaveBeenCalledWith("CR-14");
    expect(within(main).getByRole("button", { name: "prototype-results.md" })).toBeVisible();
    expect(within(main).getByText("You sent the HLD back: Add the risks.")).toBeVisible();
    expect(
      within(main).getByText("Research closed: every spike is done with a document"),
    ).toBeVisible();
  });

  it("approves a gate, leaving out an unticked child", async () => {
    const user = userEvent.setup();
    const fake = planClient().onBoard("planRule", () => ({
      case: "planned",
      value: planned(hudEpic()),
    }));
    render(<PlanWindow client={fake} openNode="PL-8" openGate />);
    const gate = await screen.findByRole("complementary", { name: "Gate" });
    expect(within(gate).getByRole("heading", { name: "Approve the HLD?" })).toBeVisible();
    expect(within(gate).getByText(/Game Lead · 17 Oct · second submission/)).toBeVisible();
    expect(within(gate).getByText("Design approved 12 Oct")).toBeVisible();
    expect(await screen.findByRole("heading", { name: "Combat HUD clarity · HLD" })).toBeVisible();
    await user.click(within(gate).getByRole("checkbox", { name: /Off-screen enemy indicators/ }));
    await user.click(within(gate).getByRole("button", { name: "Approve · create 2" }));
    expect(rulings(fake)).toMatchObject([
      { id: "PL-8", expectedVersion: 6n, approve: true, leaveOut: [1] },
    ]);
  });

  it("sends a gate back only with a note", async () => {
    const user = userEvent.setup();
    const fake = planClient().onBoard("planRule", () => ({
      case: "planned",
      value: planned(hudEpic()),
    }));
    render(<PlanWindow client={fake} openNode="PL-8" openGate />);
    const gate = await screen.findByRole("complementary", { name: "Gate" });
    const back = within(gate).getByRole("button", { name: "Send back" });
    expect(back).toBeDisabled();
    await user.type(
      within(gate).getByRole("textbox", { name: /Note to the lead/ }),
      "Cut the marker.",
    );
    await user.click(back);
    expect(rulings(fake)).toMatchObject([
      { approve: false, note: "Cut the marker.", leaveOut: [] },
    ]);
  });

  it("says why a ruling was refused", async () => {
    const user = userEvent.setup();
    const fake = planClient().onBoard("planRule", () => ({
      case: "planned",
      value: create(PlanResultSchema, {
        outcome: {
          case: "refused",
          value: {
            unmet: [
              { code: "plan.research_open", text: "Research is still open.", fix: "Finish CR-33." },
            ],
          },
        },
      }),
    }));
    render(<PlanWindow client={fake} openNode="PL-8" openGate />);
    const gate = await screen.findByRole("complementary", { name: "Gate" });
    await user.click(within(gate).getByRole("button", { name: "Approve · create 3" }));
    expect(await within(gate).findByRole("alert")).toHaveTextContent(
      "Research is still open. Finish CR-33.",
    );
  });

  it("shows a changed document's changes since approval", async () => {
    const user = userEvent.setup();
    render(<PlanWindow />);
    await user.click(await screen.findByRole("button", { name: /Combat readability/ }));
    await user.click(await screen.findByRole("button", { name: "prd.md" }));
    await user.click(screen.getByRole("tab", { name: "Changes since approval" }));
    expect(await screen.findByLabelText("Changes to prd.md since approval")).toHaveTextContent(
      "+## Risks",
    );
    await user.click(screen.getByRole("button", { name: "‹ Combat readability" }));
    expect(screen.getByRole("main", { name: "Combat readability" })).toBeVisible();
  });

  it("skips open research with a reason", async () => {
    const user = userEvent.setup();
    const fake = planClient().onBoard("planResearchSkip", () => ({
      case: "planned",
      value: planned(hudEpic()),
    }));
    render(<PlanWindow client={fake} />);
    await user.click(await screen.findByRole("button", { name: /Combat onboarding/ }));
    await user.click(await screen.findByRole("button", { name: "Skip research…" }));
    await user.type(
      screen.getByRole("textbox", { name: /Why no research is needed/ }),
      "Known from the tutorial.",
    );
    await user.click(screen.getByRole("button", { name: "Skip research" }));
    const skips = fake.boardCalls.flatMap((c) => (c.case === "planResearchSkip" ? [c.value] : []));
    expect(skips).toMatchObject([{ id: "PL-9", reason: "Known from the tutorial." }]);
  });

  it("starts a goal and an initiative under it", async () => {
    const user = userEvent.setup();
    const fake = planClient().onBoard("planCreate", (call) => ({
      case: "planned",
      value: planned({
        ...hudEpic(),
        id: "PL-20",
        title: call.case === "planCreate" ? (call.value.title ?? "") : "",
      }),
    }));
    render(<PlanWindow client={fake} />);
    await user.click(await screen.findByRole("button", { name: "+ Goal" }));
    await user.type(screen.getByRole("textbox", { name: "Title" }), "Ship the demo");
    await user.type(screen.getByRole("textbox", { name: /Deadline/ }), "Gamescom");
    await user.click(screen.getByRole("button", { name: "Start the goal" }));
    await user.click(screen.getByRole("button", { name: "+ Initiative" }));
    await user.type(screen.getByRole("textbox", { name: "Title" }), "Boss fights");
    await user.selectOptions(screen.getByRole("combobox", { name: "Lead" }), "b2");
    await user.click(screen.getByRole("button", { name: "Start the initiative" }));
    const creates = fake.boardCalls.flatMap((c) => (c.case === "planCreate" ? [c.value] : []));
    expect(creates).toMatchObject([
      { kind: PlanKind.GOAL, title: "Ship the demo", due: "Gamescom", parentId: undefined },
      { kind: PlanKind.INITIATIVE, title: "Boss fights", parentId: "PL-1", lead: "b2" },
    ]);
  });

  it("says when planning is off, or the service is older", async () => {
    render(<PlanWindow client={planClient(planTree({ enabled: false }))} />);
    expect(await screen.findByText("Planning is off for this project")).toBeVisible();
  });

  it("asks an older service for nothing", () => {
    render(<PlanWindow client={new FakeDaemon()} />);
    expect(screen.getByText("The plan needs a newer Flotilla service")).toBeVisible();
  });
});

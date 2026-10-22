import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RESEARCH_GATE_V1 } from "../../protocol/plan";
import { PLAN_PROJECT, planClient, planTree } from "../../test/planFixtures";
import PlanningSettings from "./PlanningSettings";
import type { UsePlan } from "./usePlan";

const REPO = { url: "git@github.com:me/game.git", branch: "main" };

function plan(enabled: boolean, refresh = vi.fn<() => void>(), researchGate = true): UsePlan {
  const tree = planTree({ enabled });
  tree.researchGate = researchGate;
  return {
    status: { kind: "ready", tree },
    refresh,
    apply: vi.fn<UsePlan["apply"]>(),
  };
}

describe("the planning switch", () => {
  it("is hidden without the plan", () => {
    const { container } = render(
      <PlanningSettings api={planClient()} projectId={PLAN_PROJECT} connected canControl />,
    );
    expect(container).toBeEmptyDOMElement();
    const unsupported: UsePlan = {
      status: { kind: "unsupported" },
      refresh: vi.fn<() => void>(),
      apply: vi.fn<UsePlan["apply"]>(),
    };
    const again = render(
      <PlanningSettings
        api={planClient()}
        projectId={PLAN_PROJECT}
        plan={unsupported}
        connected
        canControl
      />,
    );
    expect(again.container).toBeEmptyDOMElement();
  });

  it("needs the repository before planning can start", () => {
    render(
      <PlanningSettings
        api={planClient()}
        projectId={PLAN_PROJECT}
        plan={plan(false)}
        repo={null}
        connected
        canControl
      />,
    );
    expect(screen.getByRole("button", { name: "Turn planning on" })).toBeDisabled();
    expect(screen.getByText(/Set the project's repository above first/)).toBeVisible();
  });

  it("turns planning on and reads the plan again", async () => {
    const api = planClient().onBoard("planEnable", () => ({
      case: "plan",
      value: planTree({ enabled: true }),
    }));
    const refresh = vi.fn<() => void>();
    render(
      <PlanningSettings
        api={api}
        projectId={PLAN_PROJECT}
        plan={plan(false, refresh)}
        repo={REPO}
        connected
        canControl
      />,
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Turn planning on" }));
    });
    expect(refresh).toHaveBeenCalled();
  });

  it("shows the daemon's refusal in place", async () => {
    const api = planClient().onBoard("planEnable", () => {
      throw new Error("Planning needs the project's repository.");
    });
    render(
      <PlanningSettings
        api={api}
        projectId={PLAN_PROJECT}
        plan={plan(true)}
        repo={REPO}
        connected
        canControl
      />,
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Turn planning off" }));
    });
    expect(screen.getByRole("alert")).toHaveTextContent("Planning needs the project's repository.");
  });
});

/** A 0.24 service: the research gate, and `plan_enable` answered. */
function gated() {
  const api = planClient().onBoard("planEnable", () => ({
    case: "plan",
    value: planTree({ enabled: true }),
  }));
  api.capabilities = [...api.capabilities, RESEARCH_GATE_V1];
  return api;
}

describe("the research gate switch", () => {
  const SWITCH = { name: "Research waits for my approval" };

  it("shows only with planning on, on a service that has it", () => {
    const { rerender } = render(
      <PlanningSettings
        api={planClient()}
        projectId={PLAN_PROJECT}
        plan={plan(true)}
        repo={REPO}
        connected
        canControl
      />,
    );
    expect(screen.queryByRole("checkbox", SWITCH)).not.toBeInTheDocument();
    const api = gated();
    rerender(
      <PlanningSettings
        api={api}
        projectId={PLAN_PROJECT}
        plan={plan(false)}
        repo={REPO}
        connected
        canControl
      />,
    );
    expect(screen.queryByRole("checkbox", SWITCH)).not.toBeInTheDocument();
    rerender(
      <PlanningSettings
        api={api}
        projectId={PLAN_PROJECT}
        plan={plan(true)}
        repo={REPO}
        connected
        canControl
      />,
    );
    expect(screen.getByRole("checkbox", SWITCH)).toBeChecked();
    expect(screen.getByText(/Off, it closes by itself/)).toBeVisible();
  });

  it("turns the gate off with plan_enable and reads the plan again", async () => {
    const api = gated();
    const refresh = vi.fn<() => void>();
    render(
      <PlanningSettings
        api={api}
        projectId={PLAN_PROJECT}
        plan={plan(true, refresh)}
        repo={REPO}
        connected
        canControl
      />,
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("checkbox", SWITCH));
    });
    const enables = api.boardCalls.flatMap((c) => (c.case === "planEnable" ? [c.value] : []));
    expect(enables).toMatchObject([
      { projectId: PLAN_PROJECT, enabled: true, researchGate: false },
    ]);
    expect(refresh).toHaveBeenCalled();
  });

  it("turns it on again, and shows a refusal in place", async () => {
    const api = gated().onBoard("planEnable", () => {
      throw new Error("Only the owner turns the research gate.");
    });
    render(
      <PlanningSettings
        api={api}
        projectId={PLAN_PROJECT}
        plan={plan(true, vi.fn<() => void>(), false)}
        repo={REPO}
        connected
        canControl
      />,
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("checkbox", SWITCH));
    });
    const enables = api.boardCalls.flatMap((c) => (c.case === "planEnable" ? [c.value] : []));
    expect(enables).toMatchObject([{ researchGate: true }]);
    expect(screen.getByRole("alert")).toHaveTextContent("Only the owner turns the research gate.");
  });

  it("is the owner's alone", () => {
    const api = gated();
    api.grants = ["read", "control"];
    render(
      <PlanningSettings
        api={api}
        projectId={PLAN_PROJECT}
        plan={plan(true)}
        repo={REPO}
        connected
        canControl
      />,
    );
    expect(screen.getByRole("checkbox", SWITCH)).toBeDisabled();
  });
});

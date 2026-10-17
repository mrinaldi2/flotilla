import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { PLAN_PROJECT, planClient, planTree } from "../../test/planFixtures";
import PlanningSettings from "./PlanningSettings";
import type { UsePlan } from "./usePlan";

const REPO = { url: "git@github.com:me/game.git", branch: "main" };

function plan(enabled: boolean, refresh = vi.fn<() => void>()): UsePlan {
  return {
    status: { kind: "ready", tree: planTree({ enabled }) },
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

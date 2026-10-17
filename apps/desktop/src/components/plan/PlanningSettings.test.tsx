import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";
import { describe, expect, it } from "vitest";
import { DaemonError } from "../../protocol/connection";
import { FakeDaemon } from "../../test/fakeDaemon";
import { planClient, planTree } from "../../test/planFixtures";
import PlanningSettings from "./PlanningSettings";
import { usePlan } from "./usePlan";

function Settings(props: {
  readonly client: FakeDaemon;
  readonly hasRepo?: boolean;
  readonly noPlan?: boolean;
}): ReactElement {
  const plan = usePlan(props.client, "p1", true);
  return (
    <PlanningSettings
      api={props.client}
      projectId="p1"
      plan={props.noPlan === true ? undefined : plan}
      hasRepo={props.hasRepo ?? true}
      connected
      canControl
    />
  );
}

function enables(fake: FakeDaemon): (boolean | undefined)[] {
  return fake.boardCalls.flatMap((c) => (c.case === "planEnable" ? [c.value.enabled] : []));
}

describe("the Planning switch", () => {
  it("turns planning on, then reads the plan again", async () => {
    const user = userEvent.setup();
    const off = planTree({ enabled: false });
    const fake = planClient(off).onBoard("planEnable", () => ({ case: "plan", value: planTree() }));
    render(<Settings client={fake} />);
    await user.click(await screen.findByRole("button", { name: "Turn planning on" }));
    expect(enables(fake)).toEqual([true]);
    expect(fake.boardCalls.filter((c) => c.case === "planGet").length).toBeGreaterThan(1);
  });

  it("turns it off, and says why it couldn't", async () => {
    const user = userEvent.setup();
    const fake = planClient().onBoard("planEnable", () => {
      throw new DaemonError("forbidden", "Only the owner turns planning off.");
    });
    render(<Settings client={fake} />);
    await user.click(await screen.findByRole("button", { name: "Turn planning off" }));
    expect(enables(fake)).toEqual([false]);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Only the owner turns planning off.",
    );
  });

  it("asks for the repository before planning can start", async () => {
    render(<Settings client={planClient(planTree({ enabled: false }))} hasRepo={false} />);
    expect(await screen.findByRole("button", { name: "Turn planning on" })).toBeDisabled();
    expect(screen.getByText("Set the project's repository above first.")).toBeVisible();
  });

  it("shows nothing on an older service, or without the plan", () => {
    const { container } = render(<Settings client={new FakeDaemon()} />);
    expect(container).toBeEmptyDOMElement();
    const none = render(<Settings client={planClient()} noPlan />);
    expect(none.container).toBeEmptyDOMElement();
  });
});

import type { Story } from "@ladle/react";
import { awaitingClient, awaitingTree, hldClient, pushClient } from "../../test/planRequests";
import { PLAN_PROJECT } from "../../test/planFixtures";
import { PlanWindow } from "../../test/planStory";
import PlanningSettings from "./PlanningSettings";
import type { UsePlan } from "./usePlan";

const noop = (): void => {};

/** Research waiting for the owner: two spikes with documents, and the questions answered. */
export const ResearchAwaiting: Story = () => (
  <PlanWindow client={awaitingClient()} openNode="PL-9" openGate />
);

/** Research waiting, with an open blocking question holding the gate. */
export const ResearchHeld: Story = () => (
  <PlanWindow client={awaitingClient(true)} openNode="PL-9" openGate />
);

/** An HLD gate: the features it creates, the questions answered and the step it moves to. */
export const HldGate: Story = () => <PlanWindow client={hldClient()} openNode="PL-8" openGate />;

/** A gate whose branch isn't pushed yet: the lead's push request beside the ruling. */
export const GatePushRequest: Story = () => (
  <PlanWindow client={pushClient()} openNode="PL-8" openGate />
);

// One service for the switch: a new one each render would start over.
const SWITCH = awaitingClient();
const SETTINGS: UsePlan = {
  status: { kind: "ready", tree: awaitingTree() },
  refresh: noop,
  apply: noop,
};

/** Planning on, with the owner's switch for the research gate. */
export const ResearchGateSwitch: Story = () => (
  <div className="main" style={{ width: 640, padding: 16 }}>
    <PlanningSettings
      api={SWITCH}
      projectId={PLAN_PROJECT}
      plan={SETTINGS}
      repo={{ url: "git@github.com:me/game.git", branch: "main" }}
      connected
      canControl
    />
  </div>
);

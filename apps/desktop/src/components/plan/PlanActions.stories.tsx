import type { Story } from "@ladle/react";
import type { ReactElement } from "react";
import { hudEpic, planClient, planNodes } from "../../test/planFixtures";
import { requestsTree } from "../../test/planRequests";
import { PlanWindow } from "../../test/planStory";
import { bot } from "../../test/fixtures";
import PlanNodeActions from "./PlanNodeActions";

const DOCS = planClient();
const BOTS = [bot({ id: "b1", name: "Designer" }), bot({ id: "b2", name: "Gameplay Dev" })];
const noop = (): void => {};

/** An active goal: the owner marks it met, edits it or drops it. */
export const GoalActions: Story = () => <PlanWindow openNode="PL-1" />;

function Frame(props: {
  readonly initial: "edit" | "drop";
  readonly goal?: boolean;
}): ReactElement {
  const node = props.goal === true ? (planNodes()[0] ?? hudEpic()) : hudEpic();
  return (
    <div className="main" style={{ height: 520 }}>
      <main className="plan-main">
        <PlanNodeActions
          api={DOCS}
          node={node}
          bots={BOTS}
          leadName="Game Lead"
          canRule
          canEdit
          onChanged={noop}
          initial={props.initial}
        />
      </main>
    </div>
  );
}

/** Editing an epic: its title, summary and lead. */
export const EditEpic: Story = () => <Frame initial="edit" />;

/** Editing a goal: its outcome and its deadline. */
export const EditGoal: Story = () => <Frame initial="edit" goal />;

/** Drop…: the node and what is under it stop; their cards stay. */
export const DropNode: Story = () => <Frame initial="drop" />;

/** The lead proposed an initiative: the owner accepts or declines it. */
export const ProposedInitiative: Story = () => (
  <PlanWindow client={planClient(requestsTree())} openNode="PL-12" openGate />
);

/** The lead asks to skip an initiative's research, with its reason. */
export const SkipResearchRequest: Story = () => (
  <PlanWindow client={planClient(requestsTree())} openNode="PL-9" openGate />
);

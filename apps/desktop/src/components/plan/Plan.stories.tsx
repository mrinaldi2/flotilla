import type { Story } from "@ladle/react";
import { PLAN_PROJECT, planClient, planTree } from "../../test/planFixtures";
import { PlanWindow } from "../../test/planStory";
import PlanDocReader from "./PlanDocReader";

// One service for the reader stories: a new one each render would read again.
const DOCS = planClient();

/** The tree with its stage chips, and an epic in Design: research closed, history (canvas D1). */
export const Tree: Story = () => <PlanWindow openNode="PL-4" />;

/** Needs you: only what waits for the owner, with the nodes above it. */
export const NeedsYouFilter: Story = () => (
  <PlanWindow openNode="PL-8" initial={{ filter: "you" }} />
);

/** An HLD waiting for the owner: the document at its commit beside the ruling (canvas D2). */
export const Gate: Story = () => <PlanWindow openNode="PL-8" openGate />;

/** A feature in Build: its cards done out of all, after an approved LLD. */
export const FeatureInBuild: Story = () => <PlanWindow openNode="PL-6" />;

/** A document read in place, from the project's repository. */
export const Document: Story = () => (
  <div className="main plan-story-doc">
    <PlanDocReader
      api={DOCS}
      projectId={PLAN_PROJECT}
      path="docs/initiatives/combat-readability/prd.md"
      approvedSha="77ad310"
    />
  </div>
);

/** An approved PRD that changed on the default branch: the diff from the approved commit. */
export const ChangesSinceApproval: Story = () => (
  <div className="main plan-story-doc">
    <PlanDocReader
      api={DOCS}
      projectId={PLAN_PROJECT}
      path="docs/initiatives/combat-readability/prd.md"
      approvedSha="77ad310"
      initialTab="changes"
    />
  </div>
);

/** + Goal: what the project is for, and its deadline in the owner's words. */
export const NewGoal: Story = () => <PlanWindow initial={{ create: "goal" }} />;

/** + Initiative: under a goal, with the why that seeds its research and its lead. */
export const NewInitiative: Story = () => (
  <PlanWindow openNode="PL-2" initial={{ create: "initiative" }} />
);

/** A project whose plan is off. */
export const PlanningOff: Story = () => (
  <PlanWindow client={planClient(planTree({ enabled: false }))} />
);

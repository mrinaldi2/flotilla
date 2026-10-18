import type { Story } from "@ladle/react";
import { PlanWindow } from "../../test/planStory";
import { questionsPlan } from "../../test/questionFixtures";

/** Combat onboarding's questions under its Research: one holds the gate, one is on its spike. */
export const NodeQuestions: Story = () => (
  <PlanWindow client={questionsPlan()} openNode="PL-9" height={900} />
);

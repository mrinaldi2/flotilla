import type { Story } from "@ladle/react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { DiskBudget as Budget } from "../../protocol/cleanup";
import type { Project } from "../../protocol/entities";
import { DISK_REPORT } from "../../test/cleanupFixtures";
import { project } from "../../test/fixtures";
import { teamDaemon } from "../../test/teamFixtures";
import DiskBudget from "./DiskBudgetForm";

const noop = (): void => {};

function Frame(props: {
  readonly project: Project;
  readonly budgets: readonly Budget[];
}): ReactElement {
  const [client] = useState(() =>
    teamDaemon().onRequest("disk_report", () => ({
      type: "disk_report",
      req_id: "1",
      disk_report: { ...DISK_REPORT, budgets: props.budgets },
    })),
  );
  return (
    <div className="project-view" style={{ maxWidth: 560, padding: 16 }}>
      <DiskBudget client={client} project={props.project} connected canControl onToast={noop} />
    </div>
  );
}

/** No budget yet: the field is empty. */
export const NoBudget: Story = () => <Frame project={project()} budgets={[]} />;

/** Over its budget at the last disk report. */
export const OverBudget: Story = () => (
  <Frame
    project={project({ disk_budget_bytes: 20e9 })}
    budgets={[
      { project_id: "p1", budget_bytes: 20e9, used_bytes: 23.4e9, card_worktree_bytes: 6.1e9 },
    ]}
  />
);

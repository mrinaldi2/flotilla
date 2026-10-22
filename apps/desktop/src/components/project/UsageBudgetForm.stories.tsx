import type { Story } from "@ladle/react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { Project } from "../../protocol/entities";
import { project } from "../../test/fixtures";
import { BOOKS_BUDGET, usageDaemon } from "../../test/usageFixtures";
import UsageBudget from "./UsageBudgetForm";

const noop = (): void => {};

function Frame({ project: p }: { readonly project: Project }): ReactElement {
  const [client] = useState(() => usageDaemon({ owner: true }));
  return (
    <div className="project-view" style={{ maxWidth: 560, padding: 16 }}>
      <UsageBudget client={client} project={p} connected onToast={noop} />
    </div>
  );
}

/** No budget yet: the field is empty. */
export const NoBudget: Story = () => <Frame project={project({ name: "Books" })} />;

/** Books at 96% of its 10 M a week. */
export const WithBudget: Story = () => (
  <Frame project={project({ name: "Books", budget: BOOKS_BUDGET })} />
);

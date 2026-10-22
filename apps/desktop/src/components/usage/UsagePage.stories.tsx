import type { Story } from "@ladle/react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { UsageRange } from "../../protocol/usage";
import { bot, project } from "../../test/fixtures";
import { USAGE_NOW, usageDaemon } from "../../test/usageFixtures";
import UsagePage from "./UsagePage";

const noop = (): void => {};

function Page({ range }: { readonly range: UsageRange }): ReactElement {
  const [client] = useState(() => usageDaemon({ owner: true }));
  return (
    <div className="app" style={{ height: 720, width: 960 }}>
      <main className="main">
        <UsagePage
          client={client}
          projects={[
            project({ id: "p1", name: "PhD" }),
            project({ id: "p2", name: "Books" }),
            project({ id: "p3", name: "Flotilla" }),
          ]}
          bots={[bot({ id: "b1", name: "Phd Game Lead" })]}
          connected
          onOpenBudget={noop}
          now={USAGE_NOW}
          range={range}
        />
      </main>
    </div>
  );
}

/** The last 7 days: PhD on track, Books at 96% of its budget, Flotilla without one. */
export const SevenDays: Story = () => <Page range="7d" />;

/** Today only: no day before to compare with, and no day bars. */
export const Today: Story = () => <Page range="today" />;

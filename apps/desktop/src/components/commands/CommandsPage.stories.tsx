import type { Story } from "@ladle/react";
import { useRef, useState } from "react";
import type { ReactElement } from "react";
import {
  COMMANDS_NOW,
  commandBots,
  commandProjects,
  commandsDaemon,
  commandsReply,
} from "../../test/commandFixtures";
import type { FakeDaemon } from "../../test/fakeDaemon";
import { useClickOnce } from "../../test/storyClick";
import CommandsPage from "./CommandsPage";

const noop = (): void => {};

/** The page as the main area shows it, right of the 64 px rail. */
function Page(props: {
  /** Made once, on the first render. */
  readonly client: () => FakeDaemon;
  /** The row to select: the first whose status starts so, "◑ Running". */
  readonly click?: string;
}): ReactElement {
  const [client] = useState(props.client);
  const box = useRef<HTMLDivElement>(null);
  useClickOnce(box, props.click);
  return (
    <div ref={box} className="app" style={{ width: 960, height: "100vh" }}>
      <main className="main">
        <CommandsPage
          client={client}
          projects={commandProjects}
          bots={commandBots}
          connected
          onOpenBot={noop}
          onOpenItem={noop}
          now={COMMANDS_NOW}
        />
      </main>
    </div>
  );
}

/** Running, failed, stopped and done rows; the training run selected, Stop offered. */
export const RunningSelected: Story = () => <Page client={commandsDaemon} click="◑ Running" />;

/** The failed end-to-end run selected: its exit code and output. */
export const FailedSelected: Story = () => <Page client={commandsDaemon} click="✗ Failed" />;

/** No bot has run anything yet. */
export const Empty: Story = () => (
  <Page
    client={() =>
      commandsDaemon(() =>
        commandsReply({
          commands: [],
          summary: [{ daemon_id: "d-mac", computer_name: "mac", running: 0, failed_today: 0 }],
        }),
      )
    }
  />
);

import type { Story } from "@ladle/react";
import type { ReactElement } from "react";
import type { OwnerAction } from "../../protocol/ownerActions";
import { ownerAction, PUSH_NOW, pushAction } from "../../test/ownerActionFixtures";
import OwnerActionCard from "./OwnerActionCard";
import OwnerActionConfirm from "./OwnerActionConfirm";

const noop = (): void => undefined;

function Frame(props: { readonly children: ReactElement }): ReactElement {
  return (
    <div className="main" style={{ width: 440, padding: 16 }}>
      {props.children}
    </div>
  );
}

function Card(props: {
  readonly action: OwnerAction;
  readonly canRun?: boolean;
  readonly proposer?: string;
}): ReactElement {
  return (
    <Frame>
      <OwnerActionCard
        action={props.action}
        proposer={props.proposer ?? "DevOps"}
        canRun={props.canRun ?? true}
        onRun={noop}
        onReject={noop}
        now={PUSH_NOW}
      />
    </Frame>
  );
}

export const Waiting: Story = () => (
  <Card
    action={ownerAction({
      flags: [
        "/Users/~/Developer/gravity/scripts/stop.sh can be changed by a bot and isn't pinned",
      ],
    })}
  />
);

export const ReadOnly: Story = () => <Card action={ownerAction()} canRun={false} />;

export const Failed: Story = () => (
  <Card
    action={ownerAction({
      state: "failed",
      exit_code: 1,
      output_tail: "Error: colima is not running\n",
    })}
  />
);

export const Confirm: Story = () => (
  <Frame>
    <OwnerActionConfirm
      action={ownerAction({
        target_name: "win-pc",
        shell: "powershell",
        content: "Stop-Service docker\nGet-Process com.docker.* | Stop-Process",
        reason: "Docker holds the Flotilla folder open",
      })}
      proposer="Tester Win"
      onRun={noop}
      onCancel={noop}
      hold={false}
    />
  </Frame>
);

/** A push request (0.24.0): the exact command, what it pushes, why, and Push. */
export const PushWaiting: Story = () => <Card action={pushAction()} proposer="Automation Tester" />;

/** Pushed: the bot goes on to open its pull request. */
export const PushDone: Story = () => (
  <Card
    action={pushAction({
      state: "succeeded",
      exit_code: 0,
      run_at: "2025-03-04T15:02:00Z",
      finished_at: "2025-03-04T15:02:03Z",
    })}
    proposer="Automation Tester"
  />
);

/** Rejected, with the owner's reason. */
export const PushRejected: Story = () => (
  <Card
    action={pushAction({ state: "rejected", reject_reason: "Rebase on main first." })}
    proposer="Automation Tester"
  />
);

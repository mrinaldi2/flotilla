// The owner actions of one place in the app (H-117 R2): an item's drawer, a
// decision, or the dashboard's Needs you. Nothing renders when there are
// none. Push requests (0.24.0) show as push cards among them.

import type { ReactElement } from "react";
import type { AddToast } from "../../app/useToasts";
import type { DaemonApi } from "../../protocol/api";
import type { Bot } from "../../protocol/entities";
import { useNow } from "../control/useNow";
import OwnerActionCard from "./OwnerActionCard";
import { useOwnerActions } from "./useOwnerActions";
import type { OwnerActionScope } from "./useOwnerActions";

export default function OwnerActionList(props: {
  readonly client: DaemonApi;
  readonly connected: boolean;
  readonly scope: OwnerActionScope;
  readonly addToast: AddToast;
  readonly botName: (id: string) => string;
  readonly title?: string;
  /** The title once a push request is listed: "Commands and pushes for you". */
  readonly pushTitle?: string;
  /** "Now", for a push request's "expires in"; stories pin it. */
  readonly now?: number;
}): ReactElement | null {
  const { client, connected, scope, addToast } = props;
  const { actions, output, run, reject } = useOwnerActions(client, connected, scope, addToast);
  const clock = useNow();
  if (actions.length === 0) {
    return null;
  }
  const pushes = actions.some((a) => a.kind === "push");
  const title = (pushes ? props.pushTitle : undefined) ?? props.title;
  const canRun = connected && client.hasGrant("approve");
  const proposer = (by: string): string =>
    by.startsWith("bot:") ? props.botName(by.slice(4)) : "Flotilla";
  return (
    <section className="owner-actions" aria-label={title ?? "Commands for you to run"}>
      {title ? <h3>{title}</h3> : null}
      {actions.map((a) => (
        <OwnerActionCard
          key={a.id}
          action={a}
          output={output[a.id]}
          proposer={proposer(a.proposed_by)}
          canRun={canRun}
          onRun={(action) => void run(action)}
          onReject={(action, reason) => void reject(action, reason)}
          now={props.now ?? clock}
        />
      ))}
    </section>
  );
}

/** The open item's commands, in the board's item drawer. */
export function ItemOwnerActions(props: {
  readonly client: DaemonApi;
  readonly connected: boolean;
  readonly projectId: string;
  readonly itemId: string | null;
  readonly bots: readonly Bot[];
  readonly addToast: AddToast;
}): ReactElement | null {
  const { bots, itemId } = props;
  if (itemId === null) {
    return null;
  }
  return (
    <OwnerActionList
      client={props.client}
      connected={props.connected}
      scope={{ projectId: props.projectId, itemId }}
      addToast={props.addToast}
      botName={(id) => bots.find((b) => b.id === id)?.name ?? "a bot"}
    />
  );
}

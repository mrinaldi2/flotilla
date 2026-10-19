// A project's Team tab as its window draws it: the bots' cards with what
// the service knows of each, and from 0.23.0 the running count, its queue,
// and Wake now / Sleep now.

import type { ReactElement } from "react";
import type { DaemonState } from "../../app/useDaemonState";
import type { AddToast } from "../../app/useToasts";
import type { DaemonApi } from "../../protocol/api";
import type { Bot, Project } from "../../protocol/entities";
import type { OwnerThread, ProjectRow } from "../../protocol/gen/flotilla/home/v1/home_pb";
import { teamOn } from "../../protocol/team";
import { useTeamStatus, wakeOrSleep } from "./teamActions";
import TeamView from "./TeamView";

interface TeamTabProps {
  readonly client: DaemonApi;
  readonly daemon: DaemonState;
  readonly project: Project;
  readonly bots: readonly Bot[];
  readonly row: ProjectRow | null;
  readonly threads: readonly OwnerThread[];
  readonly now: number;
  readonly addToast: AddToast;
  readonly onCreateBot: (projectId: string) => Promise<void>;
  readonly onDeleteBot: (botId: string) => Promise<void>;
}

/** Failed deliveries per bot, for the Team cards' badge. */
function failedByBot(deliveries: readonly { readonly bot_id: string }[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const delivery of deliveries) {
    counts[delivery.bot_id] = (counts[delivery.bot_id] ?? 0) + 1;
  }
  return counts;
}

export default function TeamTab(props: TeamTabProps): ReactElement {
  const { client, daemon, project } = props;
  const team = useTeamStatus(client, daemon.connected);
  const on = teamOn(client);
  const send = (botId: string, action: "wake" | "sleep"): void => {
    const bot = daemon.bots.find((b) => b.id === botId);
    if (bot !== undefined) {
      void wakeOrSleep(client, bot, action, props.addToast);
    }
  };
  return (
    <TeamView
      bots={props.bots}
      row={props.row}
      threads={props.threads}
      unread={daemon.unreadBots}
      failed={failedByBot(daemon.failedDeliveries)}
      activity={daemon.activityByBot}
      leadBotId={project.lead_bot_id}
      now={props.now}
      canControl={daemon.canControl}
      onOpenBot={(botId) => daemon.select({ kind: "bot", botId })}
      onCreateBot={() => void props.onCreateBot(project.id)}
      onDeleteBot={(botId) => void props.onDeleteBot(botId)}
      team={team}
      allBots={daemon.bots}
      onWake={on ? (botId) => send(botId, "wake") : undefined}
      onSleep={on ? (botId) => send(botId, "sleep") : undefined}
    />
  );
}

// The Commands page (0.24.0, `commands_v1`): every command every bot ran or
// is running, across projects and computers, in place of Main chat.

import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { Bot, Project } from "../../protocol/entities";

export interface CommandsPageProps {
  readonly client: DaemonApi;
  readonly projects: readonly Project[];
  readonly bots: readonly Bot[];
  readonly connected: boolean;
  readonly onOpenBot: (botId: string) => void;
  /** Opens a card on its project's board. */
  readonly onOpenItem: (projectId: string, itemId: string) => void;
  /** "Now", for durations; stories pin it. */
  readonly now?: number;
}

export default function CommandsPage(_props: CommandsPageProps): ReactElement {
  return <div className="commands-page" />;
}

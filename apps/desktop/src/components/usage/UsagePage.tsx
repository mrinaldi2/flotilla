// The Usage page (0.24.0, `usage_v1`): tokens per project and per bot for
// today, 7 days and 30 days, each with its share and an estimated cost.

import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { Bot, Project } from "../../protocol/entities";

export interface UsagePageProps {
  readonly client: DaemonApi;
  readonly projects: readonly Project[];
  readonly bots: readonly Bot[];
  readonly connected: boolean;
  /** Opens a project's settings on its usage budget. */
  readonly onOpenBudget: (projectId: string) => void;
  /** "Now"; stories pin it. */
  readonly now?: number;
}

export default function UsagePage(_props: UsagePageProps): ReactElement {
  return <div className="usage-page" />;
}

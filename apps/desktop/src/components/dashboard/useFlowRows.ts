// What Needs you's 0.24.0 rows do: Create a bot with a role nobody holds,
// then open it (`board_flow_v1`); Extend today on a used-up budget, and
// Raise budget… in the project's Settings (`usage_v1`). Without a
// capability its rows offer nothing here.

import { useCallback, useState } from "react";
import type { AddToast } from "../../app/useToasts";
import type { DaemonApi } from "../../protocol/api";
import type { Bot } from "../../protocol/entities";
import { BOARD_FLOW_V1 } from "../../protocol/team";
import { USAGE_V1 } from "../../protocol/usage";
import { errText } from "../../util";
import { useCreateRoleBot } from "../team/createRoleBot";
import type { FlowRowActions } from "./flowRows";

interface FlowRowsOptions {
  readonly client: DaemonApi;
  readonly projectId: string;
  readonly addToast: AddToast;
  /** Reads the dashboard again once something changed. */
  readonly refresh: () => Promise<void>;
  readonly onOpenBot: (botId: string) => void;
  readonly onRaiseBudget?: (projectId: string) => void;
}

/** `budget_extend`, with the project it is on its way for. */
function useExtend(o: FlowRowsOptions): {
  readonly extend: (projectId: string) => void;
  readonly extending: string | null;
} {
  const { client, addToast, refresh } = o;
  const [extending, setExtending] = useState<string | null>(null);
  const extend = useCallback(
    (projectId: string): void => {
      setExtending(projectId);
      void (async () => {
        try {
          await client.request({ type: "budget_extend", project_id: projectId }, "project");
          addToast(
            "info",
            "Budget extended for today",
            "Its on-demand bots wake again until midnight.",
          );
          await refresh();
        } catch (error) {
          addToast("error", "Couldn't extend the budget", errText(error));
        } finally {
          setExtending(null);
        }
      })();
    },
    [client, addToast, refresh],
  );
  return { extend, extending };
}

/** The rows' actions; `canApprove` comes from Needs you itself. */
export function useFlowRows(o: FlowRowsOptions): Omit<FlowRowActions, "canApprove"> {
  const { client, refresh, onOpenBot } = o;
  const created = useCallback(
    (bot: Bot): void => {
      void refresh();
      onOpenBot(bot.id);
    },
    [refresh, onOpenBot],
  );
  const creator = useCreateRoleBot(client, o.projectId, o.addToast, created);
  const budget = useExtend(o);
  const flow = client.capabilities.includes(BOARD_FLOW_V1);
  const usage = client.capabilities.includes(USAGE_V1);
  return {
    onCreateRoleBot: flow ? creator.create : undefined,
    creatingRole: creator.creating,
    onExtendBudget: usage ? budget.extend : undefined,
    extending: budget.extending,
    onRaiseBudget: usage ? o.onRaiseBudget : undefined,
  };
}

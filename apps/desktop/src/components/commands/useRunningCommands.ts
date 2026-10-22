// How many commands run now, across every bot (0.24.0, `commands_v1`), for
// the rail's Commands badge: read once, then again on `commands_changed`.

import { useCallback, useState } from "react";
import { useLoadOnConnect } from "../../hooks/useLoadOnConnect";
import type { DaemonApi } from "../../protocol/api";
import { totals } from "./commandsText";
import { useOnCommandsChanged } from "./useOnCommandsChanged";

/**
 * The running commands of every computer, from `list_commands`' summary.
 * 0 while disabled (not connected, or an older daemon) or when the read fails.
 */
export function useRunningCommands(client: DaemonApi, enabled: boolean): number {
  const [running, setRunning] = useState(0);

  const read = useCallback(async (): Promise<void> => {
    try {
      // One row is enough: the summary counts every command, whatever the limit.
      const reply = await client.request(
        { type: "list_commands", status: "running", limit: 1 },
        "commands",
      );
      setRunning(totals(reply.summary).running);
    } catch {
      setRunning(0);
    }
  }, [client]);

  useLoadOnConnect(enabled, read);
  useOnCommandsChanged(client, enabled, read);

  return enabled ? running : 0;
}

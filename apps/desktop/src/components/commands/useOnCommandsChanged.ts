// Refetches commands when the daemon says they changed (0.24.0,
// `commands_changed`), once a burst of pushes settles: no polling.

import { useEffect } from "react";
import type { DaemonApi } from "../../protocol/api";

/** Waits this long after the last `commands_changed` before refetching. */
const SETTLE_MS = 500;

/**
 * Calls `refresh` after `commands_changed` pushes settle, while `enabled`.
 * A `projectId` or `botId` skips pushes for other projects or bots.
 */
export function useOnCommandsChanged(
  client: DaemonApi,
  enabled: boolean,
  refresh: () => Promise<void>,
  projectId: string | null = null,
  botId: string | null = null,
): void {
  useEffect(() => {
    if (!enabled) {
      return undefined;
    }
    let timer: number | undefined;
    const off = client.on("commands_changed", (push) => {
      if (
        (projectId !== null && push.project_id !== projectId) ||
        (botId !== null && push.bot_id !== botId)
      ) {
        return;
      }
      window.clearTimeout(timer);
      timer = window.setTimeout(() => void refresh(), SETTLE_MS);
    });
    return () => {
      window.clearTimeout(timer);
      off();
    };
  }, [client, enabled, refresh, projectId, botId]);
}

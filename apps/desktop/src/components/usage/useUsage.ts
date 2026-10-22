// The Usage page's data (0.24.0, `usage_v1`): one `usage_get` for the chosen
// range, read again when the range changes and, a moment later, when the
// service says a project's usage changed.

import { useCallback, useEffect, useRef, useState } from "react";
import { useLoadOnConnect } from "../../hooks/useLoadOnConnect";
import type { DaemonApi } from "../../protocol/api";
import type { Usage, UsageRange } from "../../protocol/usage";
import { errText } from "../../util";

/** How long a burst of `usage_updated` pushes is gathered into one read. */
export const USAGE_REFETCH_MS = 1500;

interface UsageState {
  readonly usage: Usage | null;
  /** Why the last read failed; null once one succeeds. */
  readonly error: string | null;
  /** Reads again now, as after a new price table. */
  readonly reload: () => Promise<void>;
}

/** Calls `again` once, `ms` after the last push in a burst. */
function useDebouncedUsagePush(client: DaemonApi, again: () => void, ms: number): void {
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => {
    const off = client.on("usage_updated", () => {
      clearTimeout(timer.current);
      timer.current = setTimeout(again, ms);
    });
    return () => {
      off();
      clearTimeout(timer.current);
    };
  }, [client, again, ms]);
}

export function useUsage(client: DaemonApi, connected: boolean, range: UsageRange): UsageState {
  const [usage, setUsage] = useState<Usage | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Only the latest read lands: a slow answer for the old range is dropped.
  const latest = useRef(0);
  const load = useCallback(async (): Promise<void> => {
    latest.current += 1;
    const mine = latest.current;
    try {
      const reply = await client.request({ type: "usage_get", range }, "usage");
      if (mine === latest.current) {
        setUsage(reply);
        setError(null);
      }
    } catch (failure) {
      if (mine === latest.current) {
        setError(errText(failure));
      }
    }
  }, [client, range]);
  useLoadOnConnect(connected, load);
  const again = useCallback((): void => {
    void load();
  }, [load]);
  useDebouncedUsagePush(client, again, USAGE_REFETCH_MS);
  return { usage, error, reload: load };
}

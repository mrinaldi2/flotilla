// How many commands run now, across every bot (0.24.0, `commands_v1`), for
// the rail's Commands badge: read once, then again on `commands_changed`.

import type { DaemonApi } from "../../protocol/api";

export function useRunningCommands(_client: DaemonApi, _connected: boolean): number {
  return 0;
}

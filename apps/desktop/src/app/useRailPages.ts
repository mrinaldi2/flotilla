// The rail's pages a service serves (0.24.0): Commands with its count of
// what runs, and Usage. An older service offers neither.

import { useRunningCommands } from "../components/commands/useRunningCommands";
import type { DaemonApi } from "../protocol/api";
import { COMMANDS_V1 } from "../protocol/commands";
import { USAGE_V1 } from "../protocol/usage";

export interface RailPages {
  readonly commands: boolean;
  readonly commandsRunning: number;
  readonly usage: boolean;
}

export function useRailPages(client: DaemonApi, connected: boolean): RailPages {
  const serves = (capability: string): boolean =>
    connected && client.capabilities.includes(capability);
  const commands = serves(COMMANDS_V1);
  const commandsRunning = useRunningCommands(client, commands);
  return { commands, commandsRunning, usage: serves(USAGE_V1) };
}

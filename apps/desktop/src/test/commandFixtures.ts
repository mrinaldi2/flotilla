// Commands across every bot (0.24.0, `commands_v1`): rows, a summary and a
// FakeDaemon that answers `list_commands` and `stop_command`.

import type { CommandsReply } from "../protocol/commands";
import { COMMANDS_V1 } from "../protocol/commands";
import type { Bot, Project } from "../protocol/entities";
import type { RequestBody } from "../protocol/requests";
import { FakeDaemon } from "./fakeDaemon";
import * as fx from "./fixtures";

type CommandRow = CommandsReply["commands"][number];

/** "Now" for the page's stories and tests: 14:20 UTC on the rows' day. */
export const COMMANDS_NOW = Date.parse("2025-03-04T14:20:00Z");

export const commandProjects: readonly Project[] = [
  fx.project({ id: "p1", name: "PhD" }),
  fx.project({ id: "p2", name: "Books" }),
];

export const commandBots: readonly Bot[] = [
  fx.bot({ id: "b1", name: "ML Engineer", project_id: "p1" }),
  fx.bot({ id: "b2", name: "Unity Dev", project_id: "p1" }),
  fx.bot({ id: "b3", name: "Automation Tester", project_id: "p1" }),
  fx.bot({ id: "b4", name: "Book Cover", project_id: "p2" }),
];

export function commandRow(over: Partial<CommandRow> = {}): CommandRow {
  return {
    id: "c1",
    bot_id: "b1",
    bot_name: "ML Engineer",
    project_id: "p1",
    daemon_id: "d-mac",
    command: "python -m dungeonflow.train --seed 7 --episodes 4000",
    description: "Train the RL baseline",
    background: true,
    status: "running",
    started_at: "2025-03-04T14:02:00Z",
    item_id: "CASS-052",
    output: "episode 2380/4000  reward -12.4  len 211\ncheckpoint saved: runs/seed7/ckpt_2400.pt",
    ...over,
  };
}

/** Running first, then newest started, as the daemon lists them. */
const commandRows: readonly CommandRow[] = [
  commandRow(),
  commandRow({
    id: "c2",
    bot_id: "b2",
    bot_name: "Unity Dev",
    command: "Unity -batchmode -projectPath game -runTests -testPlatform PlayMode",
    description: "Play-mode tests for the combat lane",
    background: false,
    started_at: "2025-03-04T14:15:00Z",
    item_id: undefined,
    task_id: "t-31",
    output: undefined,
  }),
  commandRow({
    id: "c3",
    bot_id: "b4",
    bot_name: "Book Cover",
    project_id: "p2",
    daemon_id: "d-win",
    command: "npm run render -- --cover vol-2",
    description: "Render the cover",
    started_at: "2025-03-04T13:50:00Z",
    item_id: undefined,
    output: "rendering page 3/12",
  }),
  commandRow({
    id: "c4",
    bot_id: "b3",
    bot_name: "Automation Tester",
    command: "npx playwright test tests/e2e/tour.spec.ts",
    description: "Grounding tour, end to end",
    background: false,
    status: "failed",
    exit_code: 1,
    started_at: "2025-03-04T13:41:00Z",
    ended_at: "2025-03-04T13:43:12Z",
    item_id: "CASS-049",
    output:
      "1 failed\n  tour.spec.ts:42 › the tour reaches the map\n    Timed out 30000ms waiting for '#map'",
  }),
  commandRow({
    id: "c5",
    bot_id: "b1",
    command: "python -m dungeonflow.eval --seed 3",
    description: undefined,
    status: "stopped",
    stopped_by_owner: true,
    started_at: "2025-03-04T13:20:00Z",
    ended_at: "2025-03-04T13:31:00Z",
    item_id: undefined,
    output: undefined,
  }),
  commandRow({
    id: "c6",
    bot_id: "b2",
    bot_name: "Unity Dev",
    command: "git status --short",
    description: undefined,
    background: false,
    status: "done",
    exit_code: 0,
    started_at: "2025-03-04T13:12:00Z",
    ended_at: "2025-03-04T13:12:01Z",
    item_id: undefined,
    output: "",
  }),
];

export function commandsReply(over: Partial<CommandsReply> = {}): CommandsReply {
  return {
    type: "commands",
    commands: commandRows,
    summary: [
      { daemon_id: "d-mac", computer_name: "mac", running: 2, failed_today: 1 },
      { daemon_id: "d-win", computer_name: "win-pc", running: 1, failed_today: 0 },
    ],
    ...over,
  };
}

/**
 * A daemon with `commands_v1` that lists `reply` and stops any command.
 * `grants` default to the owner's, so Stop shows.
 */
export function commandsDaemon(
  reply: (body: RequestBody) => CommandsReply = () => commandsReply(),
): FakeDaemon {
  const client = new FakeDaemon()
    .onRequest("list_commands", (body) => ({ ...reply(body), req_id: "1" }))
    .onRequest("stop_command", () => ({ type: "ok", req_id: "1" }));
  client.capabilities = [...client.capabilities, COMMANDS_V1];
  client.grants = ["read", "control", "approve"];
  return client;
}

// Commands across every bot (0.24.0, capability `commands_v1`): the
// Commands page that replaces Main chat lists what each bot ran or is
// running, across projects and linked computers.

import type { BotCommand } from "./agents";

/** The Commands page: `list_commands`, `commands_changed` and `stop_command`. */
export const COMMANDS_V1 = "commands_v1";

/** One command, with the bot and project it belongs to. Ids are the bot's own computer's. */
interface CommandRow extends BotCommand {
  readonly bot_id: string;
  readonly bot_name: string;
  readonly project_id: string;
  readonly daemon_id: string;
  /** The card it was for, when known. */
  readonly item_id?: string;
  /** The owner stopped it from the Commands page. */
  readonly stopped_by_owner?: boolean;
}

/** Per computer, for the summary on top: "7 running · 3 failed today". */
interface CommandSummary {
  readonly daemon_id: string;
  readonly computer_name: string;
  readonly running: number;
  readonly failed_today: number;
  /** The computer didn't answer: its rows are missing or from before. */
  readonly stale?: boolean;
}

type CommandStatusFilter = "running" | "failed" | "all";

export type CommandsRequestBody =
  | {
      readonly type: "list_commands";
      readonly project_id?: string;
      readonly bot_id?: string;
      readonly status?: CommandStatusFilter;
      /** Matches the command and its description, case-insensitively. */
      readonly query?: string;
      readonly limit?: number;
      /** `next_before` of the previous page. */
      readonly before?: string;
    }
  /** Owner only: kills a running background command and tells the bot. */
  | { readonly type: "stop_command"; readonly bot_id: string; readonly command_id: string };

export type CommandsReply = {
  readonly type: "commands";
  /** Running first, then newest started. */
  readonly commands: readonly CommandRow[];
  readonly summary: readonly CommandSummary[];
  readonly next_before?: string;
};

/** A bot's commands changed: refetch instead of polling. */
export type CommandsPush = {
  readonly type: "commands_changed";
  readonly bot_id: string;
  readonly project_id: string;
  readonly daemon_id: string;
};

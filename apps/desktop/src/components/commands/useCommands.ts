// The Commands page's list (0.24.0, `commands_v1`): `list_commands` with the
// page's filters, refetched on `commands_changed`, a page at a time.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useLoadOnConnect } from "../../hooks/useLoadOnConnect";
import type { DaemonApi } from "../../protocol/api";
import type { CommandsReply, CommandsRequestBody } from "../../protocol/commands";
import { errText } from "../../util";
import type { CommandRow } from "./commandsText";
import { useOnCommandsChanged } from "./useOnCommandsChanged";

type ListBody = Extract<CommandsRequestBody, { readonly type: "list_commands" }>;
export type StatusFilter = NonNullable<ListBody["status"]>;

/** Rows per page, and the most the daemon lists at once. */
const PAGE_SIZE = 50;
const MAX_LIMIT = 500;
/** Typing settles this long before the search is sent. */
const SEARCH_DELAY_MS = 300;

export interface CommandFilters {
  readonly projectId: string | null;
  readonly botId: string | null;
  readonly status: StatusFilter;
  /** As typed: sent once typing settles. */
  readonly search: string;
}

interface CommandsState {
  readonly rows: readonly CommandRow[];
  readonly summary: CommandsReply["summary"];
  readonly nextBefore: string | undefined;
  readonly loaded: boolean;
  readonly error: string | null;
}

export interface Commands extends CommandsState {
  /** Rereads the rows shown so far. */
  readonly refresh: () => Promise<void>;
  /** Reads the next page, after the last row. */
  readonly loadMore: () => Promise<void>;
}

const INITIAL: CommandsState = {
  rows: [],
  summary: [],
  nextBefore: undefined,
  loaded: false,
  error: null,
};

function listBody(
  projectId: string | null,
  botId: string | null,
  status: StatusFilter,
  query: string,
): ListBody {
  return {
    type: "list_commands",
    ...(projectId === null ? {} : { project_id: projectId }),
    ...(botId === null ? {} : { bot_id: botId }),
    status,
    ...(query === "" ? {} : { query }),
  };
}

/** `value`, once it has stopped changing for `delayMs`. */
function useSettled(value: string, delayMs: number): string {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = window.setTimeout(() => {
      setSettled(value);
    }, delayMs);
    return () => {
      window.clearTimeout(timer);
    };
  }, [value, delayMs]);
  return settled;
}

/**
 * Every bot's commands that match `filters`: read when connected and when a
 * filter changes, reread when the daemon says commands changed. A reread asks
 * for as many rows as are shown, so pages loaded with "Load more" stay.
 */
export function useCommands(
  client: DaemonApi,
  connected: boolean,
  filters: CommandFilters,
): Commands {
  const { projectId, botId, status } = filters;
  const query = useSettled(filters.search.trim(), SEARCH_DELAY_MS);
  const body = useMemo(
    () => listBody(projectId, botId, status, query),
    [projectId, botId, status, query],
  );
  const [state, setState] = useState<CommandsState>(INITIAL);
  /** Bumped by every full read, so a late answer to an older one is dropped. */
  const generation = useRef(0);
  const shown = useRef(PAGE_SIZE);

  const refresh = useCallback(async (): Promise<void> => {
    generation.current += 1;
    const mine = generation.current;
    try {
      const reply = await client.request(
        { ...body, limit: Math.min(MAX_LIMIT, shown.current) },
        "commands",
      );
      if (mine === generation.current) {
        setState({
          rows: reply.commands,
          summary: reply.summary,
          nextBefore: reply.next_before,
          loaded: true,
          error: null,
        });
      }
    } catch (failure) {
      if (mine === generation.current) {
        setState((current) => ({ ...current, loaded: true, error: errText(failure) }));
      }
    }
  }, [client, body]);

  /** A filter changed: back to the first page. */
  const reload = useCallback((): Promise<void> => {
    shown.current = PAGE_SIZE;
    return refresh();
  }, [refresh]);

  const loadMore = useCallback(async (): Promise<void> => {
    const before = state.nextBefore;
    if (before === undefined) {
      return;
    }
    const mine = generation.current;
    try {
      const reply = await client.request({ ...body, limit: PAGE_SIZE, before }, "commands");
      if (mine === generation.current) {
        shown.current += reply.commands.length;
        setState((current) => ({
          ...current,
          rows: [...current.rows, ...reply.commands],
          summary: reply.summary,
          nextBefore: reply.next_before,
          error: null,
        }));
      }
    } catch (failure) {
      if (mine === generation.current) {
        setState((current) => ({ ...current, error: errText(failure) }));
      }
    }
  }, [client, body, state.nextBefore]);

  useLoadOnConnect(connected, reload);
  useOnCommandsChanged(client, connected, refresh, projectId, botId);

  return { ...state, refresh, loadMore };
}

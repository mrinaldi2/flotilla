// The Commands page (0.24.0, `commands_v1`): every command every bot ran or
// is running, across projects and computers, in place of Main chat.

import { useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { Bot, Project } from "../../protocol/entities";
import { useNow } from "../control/useNow";
import CommandDetail from "./CommandDetail";
import type { CommandRow } from "./commandsText";
import {
  commandNote,
  computerName,
  perComputer,
  rowKey,
  startedLabel,
  statusLabel,
  took,
  totals,
} from "./commandsText";
import type { Commands, StatusFilter } from "./useCommands";
import { useCommands } from "./useCommands";

export interface CommandsPageProps {
  readonly client: DaemonApi;
  readonly projects: readonly Project[];
  readonly bots: readonly Bot[];
  readonly connected: boolean;
  readonly onOpenBot: (botId: string) => void;
  /** Opens a card on its project's board. */
  readonly onOpenItem: (projectId: string, itemId: string) => void;
  /** "Now", for durations; stories pin it. */
  readonly now?: number;
}

const STATUSES: readonly { readonly value: StatusFilter; readonly label: string }[] = [
  { value: "running", label: "Running" },
  { value: "failed", label: "Failed" },
  { value: "all", label: "All" },
];

/** The tiles on top: running, failed today, and running per computer. */
function Summary({ summary }: { readonly summary: Commands["summary"] }): ReactElement {
  const sum = totals(summary);
  return (
    <div className="commands-tiles">
      <div className="commands-tile">
        <span className="commands-tile-value commands-running">{sum.running}</span>
        <span className="commands-tile-label">running</span>
      </div>
      <div className="commands-tile">
        <span className="commands-tile-value commands-failed">{sum.failedToday}</span>
        <span className="commands-tile-label">failed today</span>
      </div>
      {summary.length === 0 ? null : (
        <div className="commands-tile commands-tile-wide">
          <span className="commands-tile-text">{perComputer(summary)}</span>
          <span className="commands-tile-label">running per computer</span>
        </div>
      )}
    </div>
  );
}

interface FiltersProps {
  readonly projects: readonly Project[];
  readonly bots: readonly Bot[];
  readonly projectId: string | null;
  readonly botId: string | null;
  readonly status: StatusFilter;
  readonly search: string;
  readonly onProject: (projectId: string | null) => void;
  readonly onBot: (botId: string | null) => void;
  readonly onStatus: (status: StatusFilter) => void;
  readonly onSearch: (search: string) => void;
}

/** "" is "All …" in a select. */
function chosen(value: string): string | null {
  return value === "" ? null : value;
}

function Filters(props: FiltersProps): ReactElement {
  const bots = props.bots.filter(
    (b) => props.projectId === null || b.project_id === props.projectId,
  );
  return (
    <div className="commands-filters" role="toolbar" aria-label="Filters">
      <label className="commands-filter">
        Project
        <select
          value={props.projectId ?? ""}
          onChange={(e) => props.onProject(chosen(e.target.value))}
        >
          <option value="">All projects</option>
          {props.projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>
      <label className="commands-filter">
        Bot
        <select value={props.botId ?? ""} onChange={(e) => props.onBot(chosen(e.target.value))}>
          <option value="">All bots</option>
          {bots.map((b) => (
            <option key={b.id} value={b.id}>
              {b.name}
            </option>
          ))}
        </select>
      </label>
      <div className="commands-status-filter" role="radiogroup" aria-label="Status">
        {STATUSES.map((s) => (
          <button
            key={s.value}
            type="button"
            role="radio"
            aria-checked={props.status === s.value}
            onClick={() => props.onStatus(s.value)}
          >
            {s.label}
          </button>
        ))}
      </div>
      <input
        type="search"
        className="commands-search"
        aria-label="Search commands"
        placeholder="Search commands…"
        value={props.search}
        onChange={(e) => props.onSearch(e.target.value)}
      />
    </div>
  );
}

interface RowProps {
  readonly row: CommandRow;
  readonly projectName: string;
  readonly computer: string | undefined;
  readonly selected: boolean;
  readonly onSelect: () => void;
  readonly now: number;
}

function Row(props: RowProps): ReactElement {
  const { row, now } = props;
  const status = statusLabel(row);
  const where = [props.projectName, props.computer ?? ""].filter((part) => part !== "").join(" · ");
  return (
    <li>
      <button
        type="button"
        className="commands-row"
        aria-pressed={props.selected}
        onClick={props.onSelect}
      >
        <span className={`commands-status commands-${row.status}`}>
          {`${status.glyph} ${status.text}`}
        </span>
        <span className="commands-cell-stack">
          <span className="commands-bot">{row.bot_name}</span>
          <span className="commands-dim">{where}</span>
        </span>
        <span className="commands-cell-stack">
          <code className="commands-line">{row.command}</code>
          <span className="commands-dim">{commandNote(row)}</span>
        </span>
        <span className="commands-dim">{startedLabel(row.started_at, now)}</span>
        <span className="commands-dim">{took(row, now)}</span>
      </button>
    </li>
  );
}

interface ListProps {
  readonly commands: Commands;
  readonly connected: boolean;
  readonly filtered: boolean;
  readonly projectName: (projectId: string) => string;
  readonly selected: string | null;
  readonly onSelect: (key: string) => void;
  readonly now: number;
}

/** Not connected, loading, or nothing to show: one line in the list. */
function listNote(props: ListProps): string | null {
  const { commands } = props;
  if (!props.connected) {
    return "Not connected.";
  }
  if (!commands.loaded) {
    return "Loading…";
  }
  if (commands.rows.length > 0 || commands.error !== null) {
    return null;
  }
  return props.filtered
    ? "No commands match these filters."
    : "Your bots have not run any commands yet.";
}

function List(props: ListProps): ReactElement {
  const { commands } = props;
  const note = listNote(props);
  return (
    <section className="commands-list" aria-label="Commands">
      <div className="commands-row commands-row-head" aria-hidden="true">
        <span>Status</span>
        <span>Bot · project</span>
        <span>Command</span>
        <span>Started</span>
        <span>Took</span>
      </div>
      {commands.error === null || !props.connected ? null : (
        <div className="commands-error" role="alert">
          <span>{commands.error}</span>
          <button type="button" className="btn btn-small" onClick={() => void commands.refresh()}>
            Try again
          </button>
        </div>
      )}
      {note === null ? null : <p className="commands-note muted">{note}</p>}
      <ul className="commands-rows">
        {commands.rows.map((row) => (
          <Row
            key={rowKey(row)}
            row={row}
            projectName={props.projectName(row.project_id)}
            computer={computerName(commands.summary, row.daemon_id)}
            selected={props.selected === rowKey(row)}
            onSelect={() => props.onSelect(rowKey(row))}
            now={props.now}
          />
        ))}
      </ul>
      {commands.nextBefore === undefined ? null : (
        <button
          type="button"
          className="btn btn-small commands-more"
          onClick={() => void commands.loadMore()}
        >
          Load more
        </button>
      )}
    </section>
  );
}

/**
 * Every process the bots run, filtered by project, bot, status and a search,
 * with the selected one's detail on the right.
 */
export default function CommandsPage(props: CommandsPageProps): ReactElement {
  const { client, projects, bots } = props;
  const ticking = useNow();
  const now = props.now ?? ticking;
  const [projectId, setProjectId] = useState<string | null>(null);
  const [botId, setBotId] = useState<string | null>(null);
  // All by default: the page is also the history of what bots ran, so the
  // finished rows show next to the running ones, as the design has them.
  const [status, setStatus] = useState<StatusFilter>("all");
  const [search, setSearch] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const commands = useCommands(client, props.connected, { projectId, botId, status, search });

  const projectName = (id: string): string => projects.find((p) => p.id === id)?.name ?? "";
  const row = commands.rows.find((r) => rowKey(r) === selected);
  const pickProject = (id: string | null): void => {
    setProjectId(id);
    if (id !== null && bots.find((b) => b.id === botId)?.project_id !== id) {
      setBotId(null);
    }
  };

  return (
    <div className="commands-page">
      <header className="commands-header">
        <div className="commands-title">
          <h1>Commands</h1>
          <span className="muted">Every process your bots run, across projects and computers</span>
        </div>
        <Summary summary={commands.summary} />
        <Filters
          projects={projects}
          bots={bots}
          projectId={projectId}
          botId={botId}
          status={status}
          search={search}
          onProject={pickProject}
          onBot={setBotId}
          onStatus={setStatus}
          onSearch={setSearch}
        />
      </header>
      <div className="commands-body">
        <List
          commands={commands}
          connected={props.connected}
          filtered={
            projectId !== null || botId !== null || status !== "all" || search.trim() !== ""
          }
          projectName={projectName}
          selected={selected}
          onSelect={setSelected}
          now={now}
        />
        {row === undefined ? (
          <aside className="commands-detail commands-detail-empty" aria-label="Command detail">
            <p className="muted">Pick a command to see its output.</p>
          </aside>
        ) : (
          <CommandDetail
            key={rowKey(row)}
            row={row}
            projectName={projectName(row.project_id)}
            computer={computerName(commands.summary, row.daemon_id)}
            canStop={client.hasGrant("approve") && row.status === "running" && row.background}
            onStop={async () => {
              await client.request(
                { type: "stop_command", bot_id: row.bot_id, command_id: row.id },
                "ok",
              );
              await commands.refresh();
            }}
            onOpenBot={() => props.onOpenBot(row.bot_id)}
            onOpenItem={(itemId) => props.onOpenItem(row.project_id, itemId)}
            now={now}
          />
        )}
      </div>
    </div>
  );
}

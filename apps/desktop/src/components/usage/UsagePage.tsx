// The Usage page (0.24.0, `usage_v1`): tokens per project and per bot for
// today, 7 days and 30 days, each with its share and an estimated cost, the
// projects' weekly budgets, and the price table the costs come from.

import { useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { Bot, Project } from "../../protocol/entities";
import { USAGE_V1 } from "../../protocol/usage";
import type { Usage, UsageRange } from "../../protocol/usage";
import { fmtTimestamp } from "../../util";
import { useNow } from "../control/useNow";
import PriceTable from "./PriceTable";
import UsageBots from "./UsageBots";
import UsageProjects from "./UsageProjects";
import { useUsage } from "./useUsage";
import {
  cacheShare,
  costText,
  dayChange,
  percentText,
  RANGES,
  rangeWords,
  tokensText,
  turnsText,
} from "./usageText";

export interface UsagePageProps {
  readonly client: DaemonApi;
  readonly projects: readonly Project[];
  readonly bots: readonly Bot[];
  readonly connected: boolean;
  /** Opens a project's settings on its usage budget. */
  readonly onOpenBudget: (projectId: string) => void;
  /** "Now"; stories pin it. */
  readonly now?: number;
  /** The period it opens on; 7 days unless a story picks another. */
  readonly range?: UsageRange;
}

function RangePicker(props: {
  readonly range: UsageRange;
  readonly onPick: (range: UsageRange) => void;
}): ReactElement {
  return (
    <div className="usage-ranges" role="radiogroup" aria-label="Period">
      {RANGES.map(({ range, label }) => (
        <button
          key={range}
          type="button"
          role="radio"
          aria-checked={props.range === range}
          className={`usage-range${props.range === range ? " usage-range-on" : ""}`}
          onClick={() => {
            props.onPick(range);
          }}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

function Tile(props: {
  readonly label: string;
  readonly value: string;
  readonly note: string;
}): ReactElement {
  return (
    <div className="usage-tile">
      <span className="usage-tile-label">{props.label}</span>
      <span className="usage-tile-value">{props.value}</span>
      <span className="usage-tile-note">{props.note}</span>
    </div>
  );
}

/** "as of 4 Mar 14:02", or Flotilla's own estimates while the table is seeded. */
function pricesWords(asOf: string): string {
  return asOf === ""
    ? "from Flotilla's estimated prices"
    : `from your price table, as of ${fmtTimestamp(asOf)}`;
}

function Tiles({ usage }: { readonly usage: Usage }): ReactElement {
  const { total, days } = usage;
  const today = days.at(-1)?.totals.tokens ?? total.tokens;
  const yesterday = days.length > 1 ? days.at(-2)?.totals.tokens : undefined;
  const cache = cacheShare(total);
  return (
    <div className="usage-tiles">
      <Tile
        label={`All projects, ${rangeWords(usage.range)}`}
        value={`${tokensText(total.tokens)} tokens`}
        note={`${costText(total.est_cost_usd)} estimated · ${turnsText(total.turns)}`}
      />
      <Tile
        label="Today"
        value={tokensText(today)}
        note={dayChange(today, yesterday) ?? "tokens so far"}
      />
      <Tile
        label="Cache reads"
        value={cache === null ? "—" : percentText(cache)}
        note="of input tokens"
      />
      <Tile
        label="Estimated cost"
        value={costText(total.est_cost_usd)}
        note={pricesWords(usage.prices_as_of)}
      />
    </div>
  );
}

function Body(props: {
  readonly usage: Usage | null;
  readonly error: string | null;
  readonly page: UsagePageProps;
  readonly now: number;
}): ReactElement {
  const { usage, error, page, now } = props;
  if (usage === null) {
    return (
      <p className="usage-dim">
        {error === null ? "Counting tokens…" : `Couldn't read usage: ${error}`}
      </p>
    );
  }
  return (
    <>
      {error === null ? null : (
        <p className="field-error" role="alert">
          Couldn&apos;t read usage again: {error}
        </p>
      )}
      <Tiles usage={usage} />
      <div className="usage-columns">
        <UsageProjects
          usage={usage}
          projects={page.projects}
          now={now}
          onOpenBudget={page.onOpenBudget}
        />
        <UsageBots usage={usage} bots={page.bots} />
      </div>
    </>
  );
}

function Counted(props: UsagePageProps): ReactElement {
  const { client, connected } = props;
  const [range, setRange] = useState<UsageRange>(props.range ?? "7d");
  const { usage, error, reload } = useUsage(client, connected, range);
  const clock = useNow();
  const now = props.now ?? clock;
  return (
    <div className="usage-page">
      <header className="usage-head" data-tauri-drag-region="deep">
        <h1 className="usage-title">Usage</h1>
        <span className="usage-dim">Claude tokens your bots used, from their sessions</span>
        <RangePicker range={range} onPick={setRange} />
      </header>
      <div className="usage-scroll">
        <Body usage={usage} error={error} page={props} now={now} />
        <PriceTable
          client={client}
          connected={connected}
          onSaved={() => {
            void reload();
          }}
        />
      </div>
    </div>
  );
}

export default function UsagePage(props: UsagePageProps): ReactElement {
  if (!props.client.capabilities.includes(USAGE_V1)) {
    return (
      <div className="usage-page">
        <header className="usage-head" data-tauri-drag-region="deep">
          <h1 className="usage-title">Usage</h1>
        </header>
        <p className="usage-scroll usage-dim">
          This computer&apos;s Flotilla doesn&apos;t count tokens yet. Update it to see usage.
        </p>
      </div>
    );
  }
  return <Counted {...props} />;
}

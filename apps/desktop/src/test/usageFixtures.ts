// Usage as the service sends it (0.24.0, `usage_v1`): three projects over a
// week, two with a weekly budget (one at 96%), their bots, and the seeded
// price table. Times are local, so the days read the same in any timezone.

import type { ProjectBudget, Usage, UsagePrice, UsageTotals } from "../protocol/usage";
import { USAGE_V1 } from "../protocol/usage";
import { FakeDaemon } from "./fakeDaemon";

/** Sunday 9 March 2025, noon: the last day of the week below. */
export const USAGE_NOW = new Date(2025, 2, 9, 12, 0).getTime();

/** Totals for `tokens`, split the way a cached Claude Code session spends them. */
export function totals(tokens: number, turns: number, cost: number): UsageTotals {
  const input = Math.round(tokens * 0.04);
  const output = Math.round(tokens * 0.06);
  const cacheWrite = Math.round(tokens * 0.08);
  return {
    input,
    output,
    cache_read: tokens - input - output - cacheWrite,
    cache_write: cacheWrite,
    tokens,
    turns,
    est_cost_usd: cost,
  };
}

const SINCE = new Date(2025, 2, 3).toISOString();

export const PHD_BUDGET: ProjectBudget = {
  weekly_tokens: 40_000_000,
  used: 29_800_000,
  since: SINCE,
  reached: false,
};

export const BOOKS_BUDGET: ProjectBudget = {
  weekly_tokens: 10_000_000,
  used: 9_600_000,
  since: SINCE,
  reached: false,
};

const WEEK = [5.1, 7.4, 6.3, 0.02, 5.8, 9.8, 6.8];

/** The last 7 days: PhD on track, Books at 96%, Flotilla without a budget. */
export function weekUsage(): Usage {
  return {
    range: "7d",
    total: totals(41_200_000, 1284, 61.87),
    projects: [
      {
        project_id: "p1",
        name: "PhD",
        totals: totals(29_800_000, 923, 44.6),
        share: 0.723,
        budget: PHD_BUDGET,
      },
      {
        project_id: "p2",
        name: "Books",
        totals: totals(9_600_000, 301, 14.4),
        share: 0.233,
        budget: BOOKS_BUDGET,
      },
      {
        project_id: "p3",
        name: "Flotilla",
        totals: totals(1_800_000, 60, 2.87),
        share: 0.044,
      },
    ],
    bots: [
      bot("b1", "Phd Game Lead", "p1", 9_100_000, 312),
      bot("b2", "Unity Dev", "p1", 6_400_000, 198),
      bot("b5", "Cover Artist", "p2", 5_100_000, 160),
      bot("b3", "ML Engineer", "p1", 4_900_000, 141),
      bot("b6", "Copy Editor", "p2", 4_500_000, 141),
      bot("b4", "Automation Tester", "p1", 3_700_000, 122),
      bot("b7", "Release Bot", "p3", 1_800_000, 60),
    ],
    days: WEEK.map((m, i) => ({
      date: `2025-03-0${3 + i}`,
      totals: totals(Math.round(m * 1e6), Math.round(m * 30), m * 1.5),
    })),
    prices_as_of: new Date(2025, 2, 4, 14, 2).toISOString(),
  };
}

function bot(
  id: string,
  name: string,
  project: string,
  tokens: number,
  turns: number,
  total = 41_200_000,
) {
  return {
    bot_id: id,
    name,
    project_id: project,
    daemon_id: "mac",
    totals: totals(tokens, turns, tokens * 1.5e-6),
    share: tokens / total,
  };
}

/** Today only: one day, so no day-before comparison and no day bars. */
function todayUsage(): Usage {
  const week = weekUsage();
  return {
    ...week,
    range: "today",
    total: totals(6_800_000, 204, 10.2),
    projects: week.projects.map((p) => ({ ...p, totals: totals(p.share * 6_800_000, 60, 3) })),
    bots: [
      bot("b1", "Phd Game Lead", "p1", 2_100_000, 61, 6_800_000),
      bot("b2", "Unity Dev", "p1", 1_500_000, 44, 6_800_000),
      bot("b5", "Cover Artist", "p2", 1_200_000, 38, 6_800_000),
      bot("b3", "ML Engineer", "p1", 1_000_000, 31, 6_800_000),
    ],
    days: week.days.slice(-1),
  };
}

/** Flotilla's own seeded table. */
const SEEDED_PRICES: readonly UsagePrice[] = [
  { model: "opus", input: 15, output: 75, cache_read: 1.5, cache_write: 18.75, estimate: true },
  { model: "sonnet", input: 3, output: 15, cache_read: 0.3, cache_write: 3.75, estimate: true },
  { model: "*", input: 3, output: 15, cache_read: 0.3, cache_write: 3.75, estimate: true },
];

/** A daemon serving usage: `usage_get` by range, the prices, and the owner's sets. */
export function usageDaemon(opts: { readonly owner?: boolean } = {}): FakeDaemon {
  const fake = new FakeDaemon()
    .onRequest("usage_get", (body) => ({
      type: "usage",
      req_id: "1",
      ...(body.type === "usage_get" && body.range === "today" ? todayUsage() : weekUsage()),
    }))
    .onRequest("usage_prices", () => ({
      type: "usage_prices",
      req_id: "1",
      prices: SEEDED_PRICES,
      prices_as_of: null,
    }))
    .onRequest("set_usage_prices", (body) => ({
      type: "usage_prices",
      req_id: "1",
      prices: body.type === "set_usage_prices" ? body.prices : [],
      prices_as_of: new Date(2025, 2, 9, 11, 0).toISOString(),
    }));
  fake.capabilities = [...fake.capabilities, USAGE_V1];
  if (opts.owner === true) {
    fake.grants = ["read", "control", "approve"];
  }
  return fake;
}

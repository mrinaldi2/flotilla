// Usage and budgets (0.24.0, capability `usage_v1`): tokens per project and
// bot from the bots' transcripts, an estimated cost from the owner's price
// table, and a weekly token budget per project.

type UsageRange = "today" | "7d" | "30d";

interface UsageTotals {
  readonly input: number;
  readonly output: number;
  readonly cache_read: number;
  readonly cache_write: number;
  /** Every kind summed. */
  readonly tokens: number;
  readonly turns: number;
  /** An estimate, from the price table. */
  readonly est_cost_usd: number;
}

export interface ProjectBudget {
  readonly weekly_tokens: number;
  /** Used over the last 7 days. */
  readonly used: number;
  readonly since: string;
  /** Extend today: on-demand bots wake past the budget until then. */
  readonly extended_until?: string;
  /** Its on-demand bots don't wake. */
  readonly reached: boolean;
}

interface UsagePrice {
  readonly model: string;
  /** USD per million tokens. */
  readonly input: number;
  readonly output: number;
  readonly cache_read: number;
  readonly cache_write: number;
}

interface Usage {
  readonly range: UsageRange;
  readonly total: UsageTotals;
  readonly projects: readonly {
    readonly project_id: string;
    readonly name: string;
    readonly totals: UsageTotals;
    /** Of the total, 0 to 1. */
    readonly share: number;
    readonly budget?: ProjectBudget;
  }[];
  readonly bots: readonly {
    readonly bot_id: string;
    readonly name: string;
    readonly project_id: string;
    readonly daemon_id: string;
    readonly totals: UsageTotals;
    readonly share: number;
  }[];
  /** Local dates, oldest first. */
  readonly days: readonly { readonly date: string; readonly totals: UsageTotals }[];
  readonly prices_as_of: string;
}

export type UsageRequestBody =
  | {
      readonly type: "usage_get";
      readonly project_id?: string;
      readonly bot_id?: string;
      readonly range: UsageRange;
    }
  /** Owner only; null removes the budget. */
  | {
      readonly type: "set_project_budget";
      readonly project_id: string;
      readonly weekly_tokens: number | null;
    }
  /** Owner only: on-demand bots wake past the budget until local midnight. */
  | { readonly type: "budget_extend"; readonly project_id: string }
  | { readonly type: "usage_prices" }
  /** Owner only. */
  | { readonly type: "set_usage_prices"; readonly prices: readonly UsagePrice[] };

export type UsageReply =
  | ({ readonly type: "usage" } & Usage)
  | { readonly type: "usage_prices"; readonly prices: readonly UsagePrice[] };

/** A project's usage changed; at most once a minute while its bots work. */
export type UsagePush = { readonly type: "usage_updated"; readonly project_id: string };

// Usage and budgets (0.24.0, capability `usage_v1`): tokens per project and
// bot from the bots' transcripts, an estimated cost from the owner's price
// table, and a weekly token budget per project.

/** `usage_get`, budgets, the price table and `usage_updated`. */
export const USAGE_V1 = "usage_v1";

export type UsageRange = "today" | "7d" | "30d";

export interface UsageTotals {
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

export interface UsagePrice {
  /** A model family matched against the model's name; `*` is the fallback. */
  readonly model: string;
  /** USD per million tokens. */
  readonly input: number;
  readonly output: number;
  readonly cache_read: number;
  readonly cache_write: number;
  /** Flotilla's own seeded estimate, not a price the owner set. */
  readonly estimate?: boolean;
}

export interface Usage {
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
  /** When the owner last set the price table; empty while it is seeded. */
  readonly prices_as_of: string;
}

export type UsageRequestBody =
  | {
      readonly type: "usage_get";
      readonly project_id?: string;
      readonly bot_id?: string;
      readonly range: UsageRange;
    }
  /** Owner only; a whole number above 0, or null to remove it. Answers `project`. */
  | {
      readonly type: "set_project_budget";
      readonly project_id: string;
      readonly weekly_tokens: number | null;
    }
  /** Owner only: on-demand bots wake past the budget until local midnight. Answers `project`. */
  | { readonly type: "budget_extend"; readonly project_id: string }
  | { readonly type: "usage_prices" }
  /** Owner only: replaces the whole table (at most 64 rows). Answers `usage_prices`. */
  | { readonly type: "set_usage_prices"; readonly prices: readonly UsagePrice[] };

export type UsageReply =
  | ({ readonly type: "usage" } & Usage)
  | {
      readonly type: "usage_prices";
      readonly prices: readonly UsagePrice[];
      /** When the owner last set it; null while it is seeded. */
      readonly prices_as_of?: string | null;
    };

/** A project's usage changed; at most once a minute while its bots work. */
export type UsagePush = { readonly type: "usage_updated"; readonly project_id: string };

// Owner-facing words for token usage (0.24.0, `usage_v1`): "41.2 M" tokens,
// "≈ $12.40" estimates, a budget's state and the days of a range. Every state
// is spelled out, so the page reads the same without colour.

import type { ProjectBudget, UsageRange, UsageTotals } from "../../protocol/usage";

/** "41.2", "40": one decimal, none when it is a whole number. */
function oneDecimal(n: number): string {
  const rounded = Math.round(n * 10) / 10;
  return Number.isInteger(rounded) ? String(rounded) : rounded.toFixed(1);
}

/** "1.2 B", "41.2 M", "940 k", "512": tokens, short. */
export function tokensText(tokens: number): string {
  const n = Math.max(0, tokens);
  if (n >= 999_950_000) {
    return `${oneDecimal(n / 1e9)} B`;
  }
  if (n >= 999_500) {
    return `${oneDecimal(n / 1e6)} M`;
  }
  if (n >= 9_950) {
    return `${Math.round(n / 1e3)} k`;
  }
  if (n >= 1_000) {
    return `${oneDecimal(n / 1e3)} k`;
  }
  return String(Math.round(n));
}

const USD = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD" });

/** "≈ $12.40": every cost here is an estimate from the price table. */
export function costText(usd: number): string {
  if (usd > 0 && usd < 0.005) {
    return "≈ under $0.01";
  }
  return `≈ ${USD.format(usd)}`;
}

/** "74%": a 0..1 share, whole. */
export function percentText(share: number): string {
  return `${Math.round(share * 100)}%`;
}

/** "1,284 turns". */
export function turnsText(turns: number): string {
  return `${turns.toLocaleString("en-US")} ${turns === 1 ? "turn" : "turns"}`;
}

/** The period radio's words, and the tile's "All projects, …". */
export const RANGES: readonly { readonly range: UsageRange; readonly label: string }[] = [
  { range: "today", label: "Today" },
  { range: "7d", label: "7 days" },
  { range: "30d", label: "30 days" },
];

export function rangeWords(range: UsageRange): string {
  switch (range) {
    case "today":
      return "today";
    case "7d":
      return "the last 7 days";
    default:
      return "the last 30 days";
  }
}

/** Cache reads as a share of every input token: what the cache saved. */
export function cacheShare(totals: UsageTotals): number | null {
  const input = totals.input + totals.cache_read + totals.cache_write;
  return input > 0 ? totals.cache_read / input : null;
}

/** "↓ 31% on yesterday"; null when there's no yesterday to compare with. */
export function dayChange(today: number, yesterday: number | undefined): string | null {
  if (yesterday === undefined || yesterday <= 0) {
    return null;
  }
  const change = Math.round(((today - yesterday) / yesterday) * 100);
  if (change === 0) {
    return "Same as yesterday";
  }
  return change > 0 ? `↑ ${change}% on yesterday` : `↓ ${-change}% on yesterday`;
}

/** A local "YYYY-MM-DD" as that day's midnight. */
function localDate(date: string): Date {
  const [y = 0, m = 1, d = 1] = date.split("-").map(Number);
  return new Date(y, m - 1, d);
}

function sameDay(a: Date, b: Date): boolean {
  return a.toDateString() === b.toDateString();
}

/** "Today", "Mon" in a week, "Mar 4" in a month. */
export function dayLabel(date: string, now: number, range: UsageRange): string {
  const day = localDate(date);
  if (sameDay(day, new Date(now))) {
    return "Today";
  }
  return range === "30d"
    ? day.toLocaleDateString("en-US", { day: "numeric", month: "short" })
    : day.toLocaleDateString("en-US", { weekday: "short" });
}

/** Within the budget's week, used over the weekly tokens, 0 and up. */
export function budgetShare(budget: ProjectBudget): number {
  return budget.weekly_tokens > 0 ? budget.used / budget.weekly_tokens : 0;
}

/** "29.8 M of 40 M". */
export function budgetUsed(used: number, weekly: number): string {
  return `${tokensText(used)} of ${tokensText(weekly)}`;
}

type BudgetTone = "ok" | "near" | "reached" | "extended";

interface BudgetState {
  readonly tone: BudgetTone;
  readonly words: string;
}

/** "Thu 00:00": when an extension ends. */
function untilText(at: string, now: number): string {
  const date = new Date(at);
  const time = date.toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  });
  return sameDay(date, new Date(now))
    ? time
    : `${date.toLocaleDateString("en-US", { weekday: "short" })} ${time}`;
}

/** A used bar's colour: reached, nearly there from 80%, else on track. */
export function budgetTone(budget: ProjectBudget): "ok" | "near" | "reached" {
  if (budget.reached) {
    return "reached";
  }
  return budgetShare(budget) >= 0.8 ? "near" : "ok";
}

const TONE_WORDS = { ok: "On track", near: "Nearly there", reached: "Reached" } as const;

/** On track, nearly there from 80%, reached, or extended until a time. */
export function budgetState(budget: ProjectBudget, now: number): BudgetState {
  const tone = budgetTone(budget);
  const until = budget.extended_until;
  if (tone !== "reached" && until !== undefined && Date.parse(until) > now) {
    return { tone: "extended", words: `Extended until ${untilText(until, now)}` };
  }
  return { tone, words: TONE_WORDS[tone] };
}

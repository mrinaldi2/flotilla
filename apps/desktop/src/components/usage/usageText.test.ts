import { describe, expect, it } from "vitest";
import { BOOKS_BUDGET, PHD_BUDGET, totals, USAGE_NOW } from "../../test/usageFixtures";
import {
  budgetState,
  budgetTone,
  budgetUsed,
  cacheShare,
  costText,
  dayChange,
  dayLabel,
  percentText,
  rangeWords,
  tokensText,
  turnsText,
} from "./usageText";

describe("usage words", () => {
  it("says tokens short", () => {
    expect(tokensText(41_234_567)).toBe("41.2 M");
    expect(tokensText(40_000_000)).toBe("40 M");
    expect(tokensText(940_100)).toBe("940 k");
    expect(tokensText(999_600)).toBe("1 M");
    expect(tokensText(1_500)).toBe("1.5 k");
    expect(tokensText(512)).toBe("512");
    expect(tokensText(-3)).toBe("0");
    expect(tokensText(2_340_000_000)).toBe("2.3 B");
  });

  it("marks every cost as an estimate", () => {
    expect(costText(12.4)).toBe("≈ $12.40");
    expect(costText(1234.5)).toBe("≈ $1,234.50");
    expect(costText(0)).toBe("≈ $0.00");
    expect(costText(0.001)).toBe("≈ under $0.01");
  });

  it("says shares, turns and ranges", () => {
    expect(percentText(0.745)).toBe("75%");
    expect(turnsText(1284)).toBe("1,284 turns");
    expect(turnsText(1)).toBe("1 turn");
    expect(rangeWords("today")).toBe("today");
    expect(rangeWords("7d")).toBe("the last 7 days");
    expect(rangeWords("30d")).toBe("the last 30 days");
  });

  it("counts cache reads against every input token", () => {
    const t = totals(1_000_000, 1, 1);
    expect(cacheShare(t)).toBeCloseTo(0.82 / 0.94);
    expect(cacheShare(totals(0, 0, 0))).toBeNull();
  });

  it("compares today with the day before, when there is one", () => {
    expect(dayChange(6.8, 9.8)).toBe("↓ 31% on yesterday");
    expect(dayChange(12, 10)).toBe("↑ 20% on yesterday");
    expect(dayChange(10, 10)).toBe("Same as yesterday");
    expect(dayChange(10, undefined)).toBeNull();
    expect(dayChange(10, 0)).toBeNull();
  });

  it("names the days: today, a weekday, or a date in a month", () => {
    expect(dayLabel("2025-03-09", USAGE_NOW, "7d")).toBe("Today");
    expect(dayLabel("2025-03-03", USAGE_NOW, "7d")).toBe("Mon");
    expect(dayLabel("2025-02-14", USAGE_NOW, "30d")).toBe("Feb 14");
  });

  it("words a budget's state", () => {
    expect(budgetUsed(29_800_000, 40_000_000)).toBe("29.8 M of 40 M");
    expect(budgetState(PHD_BUDGET, USAGE_NOW)).toEqual({ tone: "ok", words: "On track" });
    expect(budgetState(BOOKS_BUDGET, USAGE_NOW)).toEqual({ tone: "near", words: "Nearly there" });
    const reached = { ...BOOKS_BUDGET, used: 10_200_000, reached: true };
    expect(budgetState(reached, USAGE_NOW)).toEqual({ tone: "reached", words: "Reached" });
    expect(budgetTone(reached)).toBe("reached");
    const midnight = new Date(2025, 2, 10, 0, 0).toISOString();
    const extended = { ...BOOKS_BUDGET, used: 10_200_000, extended_until: midnight };
    expect(budgetState(extended, USAGE_NOW)).toEqual({
      tone: "extended",
      words: "Extended until Mon 00:00",
    });
    const tonight = new Date(2025, 2, 9, 23, 30).toISOString();
    expect(budgetState({ ...extended, extended_until: tonight }, USAGE_NOW).words).toBe(
      "Extended until 23:30",
    );
    // An extension that ended reads as the budget's own state.
    const past = new Date(2025, 2, 8).toISOString();
    expect(budgetState({ ...extended, extended_until: past }, USAGE_NOW).tone).toBe("near");
    expect(budgetTone({ ...PHD_BUDGET, weekly_tokens: 0 })).toBe("ok");
  });
});

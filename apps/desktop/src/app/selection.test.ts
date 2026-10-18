import { describe, expect, it } from "vitest";
import { isProjectTab, moreTabs, primaryTabs } from "./selection";

describe("a project window's tabs", () => {
  it("keeps six in the bar, Plan after Overview, the rest under More", () => {
    expect(primaryTabs({ prs: true, plan: true })).toEqual([
      "overview",
      "plan",
      "board",
      "prs",
      "team",
      "releases",
    ]);
    expect(moreTabs({ prs: true, plan: true })).toEqual(["meetings", "conversations", "settings"]);
  });

  it("is unchanged without the plan", () => {
    expect(primaryTabs({ prs: true, plan: false })).toEqual([
      "overview",
      "board",
      "prs",
      "team",
      "releases",
      "meetings",
    ]);
    expect(primaryTabs({ prs: false, plan: false })).not.toContain("prs");
    expect(moreTabs({ prs: false, plan: false })).toEqual(["conversations", "settings"]);
    expect(moreTabs({ prs: false, plan: true })).toEqual(["conversations", "settings"]);
    expect(isProjectTab("plan")).toBe(true);
  });

  it("puts Questions under More once the service serves them", () => {
    expect(moreTabs({ prs: true, plan: true, questions: true })).toEqual([
      "meetings",
      "questions",
      "conversations",
      "settings",
    ]);
    expect(primaryTabs({ prs: true, plan: true, questions: true })).not.toContain("questions");
  });
});

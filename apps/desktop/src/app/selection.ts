import type { BotTab } from "../components/bot/BotTabs";

/**
 * The tabs of a project window (UX-024 §2, UX-051 §1): the first six are in
 * the tab bar on ⌘1…⌘6; the rest sit under More. Pull requests shows only when
 * the service serves them, and Plan (0.21.0) only on a project whose plan is
 * on; with both, Meetings moves under More. Questions (0.22.0) sits under
 * More when the service serves them.
 */
const PROJECT_TABS = [
  "overview",
  "plan",
  "board",
  "prs",
  "team",
  "releases",
  "meetings",
  "questions",
  "conversations",
  "settings",
] as const;

export type ProjectTab = (typeof PROJECT_TABS)[number];

/** The tabs the bar can hold, in order; the rest are always under More. */
const BAR_TABS: readonly ProjectTab[] = PROJECT_TABS.slice(0, 7);
const ALWAYS_MORE: readonly ProjectTab[] = PROJECT_TABS.slice(7);
const BAR_SIZE = 6;

/** Which optional tabs show: Pull requests, the Plan, and Questions. */
export interface TabsShown {
  readonly prs: boolean;
  readonly plan: boolean;
  readonly questions?: boolean;
}

function shownTabs(shown: TabsShown): readonly ProjectTab[] {
  return BAR_TABS.filter((tab) => (tab !== "prs" || shown.prs) && (tab !== "plan" || shown.plan));
}

/** The tabs in the tab bar itself, in ⌘1… order. */
export function primaryTabs(shown: TabsShown): readonly ProjectTab[] {
  return shownTabs(shown).slice(0, BAR_SIZE);
}

/** The tabs under More: what the bar has no room for, then Conversations and Settings. */
export function moreTabs(shown: TabsShown): readonly ProjectTab[] {
  const always = ALWAYS_MORE.filter((tab) => tab !== "questions" || shown.questions === true);
  return [...shownTabs(shown).slice(BAR_SIZE), ...always];
}

/** What the main pane is currently showing. */
export type Selection =
  /** The projects home, ranked by what needs the owner: where the app opens (H-133). */
  | { readonly kind: "home" }
  /**
   * A bot's page. `tab: "chat"` opens it on its Chat, as Reply on a report
   * does (H-192); without it, the page opens on its first tab. History
   * records the tab the owner moved to, so going back reopens it. `press`
   * counts those asks, so a second one lands on Chat even when the page is
   * already open and the owner moved to another tab (UX-034).
   */
  | {
      readonly kind: "bot";
      readonly botId: string;
      readonly tab?: BotTab;
      readonly press?: number;
    }
  /**
   * A project window. Without `tab` it reopens on the tab last used there,
   * which is the Dashboard the first time. `item` opens that card's drawer
   * on the Board ("Open on the board", UX-035 §4).
   */
  | {
      readonly kind: "project";
      readonly projectId: string;
      readonly tab?: ProjectTab;
      readonly item?: string;
      /** Pull requests: open this PR (Needs you's Review…, H-277). */
      readonly pr?: number;
      /** With `pr`: open its delta since your approval (a Re-check row). */
      readonly recheck?: boolean;
      /** Settings: focus this section ("Change" on a PR's Owner review line). */
      readonly section?: "owner_review" | "budget";
      /** Board: show this board (0.19.0); an `item`'s own board wins. */
      readonly board?: string;
      /** With `board`: open its settings ("Change it…" on a lead's proposal). */
      readonly boardSettings?: boolean;
      /** Plan: open this node (0.21.0); with `gate`, on its gate for the owner's ruling. */
      readonly planNode?: string;
      readonly gate?: boolean;
    }
  /**
   * The Control center. `decisionId` opens straight onto one record, which is
   * what a notification or a palette entry hands over.
   */
  | { readonly kind: "control"; readonly decisionId?: string }
  /** Every bot's commands, across projects and computers (0.24.0, `commands_v1`). */
  | { readonly kind: "commands" }
  /** Tokens per project and bot, and the price table (0.24.0, `usage_v1`). */
  | { readonly kind: "usage" };

/** Narrows a stored string to a project tab. */
export function isProjectTab(value: string): value is ProjectTab {
  return PROJECT_TABS.some((tab) => tab === value);
}

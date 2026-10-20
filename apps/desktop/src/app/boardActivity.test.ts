import { create } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import {
  BoardActivityKind as Kind,
  BoardActivitySchema,
} from "../protocol/gen/flotilla/home/v1/home_pb";
import type { BoardActivity } from "../protocol/gen/flotilla/home/v1/home_pb";
import type { BoardActivityPrefs } from "../prefs";
import {
  ACTIVITY_TTL_MS,
  EMPTY_STACK,
  GROUP_WINDOW_MS,
  MAX_ACTIVITY_TOASTS,
  actorLabel,
  addNotice,
  allowed,
  dismissToast,
  expireStack,
  noticeFromActivity,
  noticeFromBotState,
} from "./boardActivity";
import type { ActivityNotice, ActivityStack } from "./boardActivity";

const names = (id: string): string | undefined => (id === "b1" ? "New Bot" : undefined);

type ActivityInit = MessageInitShape<typeof BoardActivitySchema>;

function activity(over: ActivityInit = {}): BoardActivity {
  return create(BoardActivitySchema, {
    projectId: "p1",
    itemId: "P2-001",
    itemTitle: "Test",
    kind: Kind.MOVED,
    actor: "bot:b1",
    fromName: "To do",
    toName: "Doing",
    ...over,
  });
}

function notice(over: ActivityInit = {}): ActivityNotice {
  const made = noticeFromActivity(activity(over), names);
  if (made === null) {
    throw new Error("expected a notice");
  }
  return made;
}

function addAll(notices: readonly ActivityNotice[], at: number): ActivityStack {
  return notices.reduce((stack, n) => addNotice(stack, n, at), EMPTY_STACK);
}

const ALL_ON: BoardActivityPrefs = {
  moved: true,
  assigned: true,
  refused: true,
  turns: true,
  drafts: true,
  checks: true,
};

describe("board activity notices", () => {
  it("say who did what with a glyph and a word", () => {
    const moved = notice();
    expect(moved.actorLabel).toBe("New Bot");
    expect(moved.line).toEqual({ glyph: "→", word: "Moved", detail: "To do → Doing" });
    const refused = notice({
      kind: Kind.MOVE_REFUSED,
      unmet: ["Only the owner can move it."],
    });
    expect(refused.pref).toBe("refused");
    expect(refused.line.word).toBe("Couldn't move");
    expect(refused.line.detail).toBe("to Doing · Only the owner can move it.");
    const picked = notice({ kind: Kind.ASSIGNED, assignee: "b1", assigneeName: "New Bot" });
    expect(picked.line.word).toBe("Picked up");
    const assigned = notice({
      kind: Kind.ASSIGNED,
      actor: "user",
      assignee: "b2",
      assigneeName: "Dev",
    });
    expect([assigned.actorLabel, assigned.line.detail]).toEqual(["You", "to Dev"]);
    expect(notice({ kind: Kind.DRAFT_UPDATED, note: "description" }).line).toEqual({
      glyph: "✎",
      word: "Updated the draft",
      detail: "description",
    });
  });

  it("say what the board keeper did, as Automatic", () => {
    const asked = notice({
      kind: Kind.CHECKED,
      actor: "daemon",
      assignee: "b1",
      assigneeName: "New Bot",
      note: "In Doing, nothing has happened for 2 h",
    });
    expect(asked.pref).toBe("checks");
    expect(asked.actorLabel).toBe("Automatic");
    expect(asked.line).toEqual({
      glyph: "◷",
      word: "Asked New Bot",
      detail: "In Doing, nothing has happened for 2 h",
    });
    const row = notice({ kind: Kind.CHECKED, actor: "daemon", note: "Can't move to Done" });
    expect(row.line.word).toBe("Needs you");
    expect(allowed(row, { ...ALL_ON, checks: false })).toBe(false);
  });

  it("skip what this app asked for itself, and kinds it doesn't know", () => {
    expect(noticeFromActivity(activity({ fromThisClient: true }), names)).toBeNull();
    expect(noticeFromActivity(activity({ kind: Kind.UNSPECIFIED }), names)).toBeNull();
  });

  it("name the owner elsewhere and the daemon", () => {
    expect(actorLabel("device:d1", "iPhone", names)).toBe("You, on iPhone");
    expect(actorLabel("daemon", "", names)).toBe("Automatic");
    expect(actorLabel("bot:gone", "Old Bot", names)).toBe("Old Bot");
  });

  it("stay off when their kind is turned off", () => {
    expect(allowed(notice(), ALL_ON)).toBe(true);
    expect(allowed(notice(), { ...ALL_ON, moved: false })).toBe(false);
  });
});

describe("the activity stack", () => {
  it("folds one actor's notices about one card into one toast", () => {
    const stack = addAll([notice(), notice({ kind: Kind.DRAFT_UPDATED, note: "title" })], 0);
    expect(stack.toasts).toHaveLength(1);
    expect(stack.toasts[0]?.lines.map((l) => l.word)).toEqual(["Moved", "Updated the draft"]);
  });

  it("starts a new toast once the window has passed, or for another card or actor", () => {
    const first = addNotice(EMPTY_STACK, notice(), 0);
    const later = addNotice(first, notice(), GROUP_WINDOW_MS + 1);
    expect(later.toasts).toHaveLength(2);
    const other = addAll([notice(), notice({ itemId: "P2-002" }), notice({ actor: "user" })], 0);
    expect(other.toasts).toHaveLength(3);
  });

  it("shows at most three, newest on top", () => {
    const many = ["A-1", "A-2", "A-3", "A-4", "A-5"].map((itemId) => notice({ itemId }));
    const stack = addAll(many, 0);
    expect(stack.toasts).toHaveLength(MAX_ACTIVITY_TOASTS);
    expect(stack.toasts.map((t) => t.itemId)).toEqual(["A-5", "A-4", "A-3"]);
  });

  it("caps a burst on one card at a few lines", () => {
    const burst = Array.from({ length: 9 }, (_, i) => notice({ note: `step ${i}` }));
    const lines = addAll(burst, 0).toasts[0]?.lines ?? [];
    expect(lines).toHaveLength(4);
    expect(lines.at(-1)?.detail).toContain("step 8");
  });

  it("expires toasts nothing updated, and dismisses by id", () => {
    const stack = addNotice(addNotice(EMPTY_STACK, notice(), 0), notice({ itemId: "B" }), 5000);
    const swept = expireStack(stack, ACTIVITY_TTL_MS + 1);
    expect(swept.toasts.map((t) => t.itemId)).toEqual(["B"]);
    expect(expireStack(swept, ACTIVITY_TTL_MS + 2)).toBe(swept);
    expect(dismissToast(swept, swept.toasts[0]?.id ?? 0).toasts).toEqual([]);
  });
});

describe("turn notices", () => {
  const bot = { name: "New Bot", projectId: "p1" };

  it("say a bot started and finished on a card, once per turn", () => {
    const open = new Map<string, string>();
    const start = noticeFromBotState(
      { bot_id: "b1", state: "working", item_id: "P2-001", item_title: "Test" },
      open,
      bot,
    );
    expect(start?.line.word).toBe("Started");
    expect(start?.itemTitle).toBe("Test");
    // An approval wait and back inside the turn isn't a new start.
    expect(
      noticeFromBotState(
        { bot_id: "b1", state: "waiting_for_approval", item_id: "P2-001" },
        open,
        bot,
      ),
    ).toBeNull();
    expect(
      noticeFromBotState({ bot_id: "b1", state: "working", item_id: "P2-001" }, open, bot),
    ).toBeNull();
    const done = noticeFromBotState({ bot_id: "b1", state: "ready" }, open, bot);
    expect([done?.line.word, done?.itemId, done?.pref]).toEqual(["Finished", "P2-001", "turns"]);
  });

  it("say the bot got the owner's answer when the answer started the turn", () => {
    const open = new Map<string, string>();
    const start = noticeFromBotState(
      { bot_id: "b1", state: "working", item_id: "ASL-004", cause: "answer" },
      open,
      bot,
    );
    expect([start?.line.word, start?.line.detail, start?.itemId]).toEqual([
      "Got your answer",
      "working on it",
      "ASL-004",
    ]);
    const done = noticeFromBotState(
      { bot_id: "b1", state: "ready", item_id: "ASL-004" },
      open,
      bot,
    );
    expect(done?.line.word).toBe("Finished");
  });

  it("say nothing about a turn on no card", () => {
    const open = new Map<string, string>();
    expect(noticeFromBotState({ bot_id: "b1", state: "working" }, open, bot)).toBeNull();
    expect(noticeFromBotState({ bot_id: "b1", state: "ready" }, open, bot)).toBeNull();
  });

  it("say a turn finished on the card the bot changed during it", () => {
    const open = new Map<string, string>();
    expect(noticeFromBotState({ bot_id: "b1", state: "working" }, open, bot)).toBeNull();
    const done = noticeFromBotState({ bot_id: "b1", state: "ready", item_id: "P2-002" }, open, bot);
    expect([done?.line.word, done?.itemId]).toEqual(["Finished", "P2-002"]);
  });
});

import type { JsonObject } from "@bufbuild/protobuf";
import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import ActivityToasts from "../components/ActivityToasts";
import type { BoardActivityPrefs } from "../prefs";
import { bot } from "../test/fixtures";
import { FakeDaemon } from "../test/fakeDaemon";
import { ACTIVITY_TTL_MS } from "./boardActivity";
import type { ActivityToast } from "./boardActivity";
import type { Selection } from "./selection";
import { useBoardActivity } from "./useBoardActivity";

const ALL_ON: BoardActivityPrefs = {
  moved: true,
  assigned: true,
  refused: true,
  turns: true,
  drafts: true,
};

/** A `board_activity` push as the daemon writes it: proto3 JSON, proto field names. */
function moved(over: JsonObject = {}): { activity: JsonObject } {
  return {
    activity: {
      project_id: "p1",
      item_id: "P2-001",
      item_title: "Test",
      kind: "BOARD_ACTIVITY_KIND_MOVED",
      actor: "bot:b1",
      from_name: "To do",
      to_name: "Doing",
      ...over,
    },
  };
}

let now = 0;

beforeEach(() => {
  vi.useFakeTimers();
  now = 0;
});

afterEach(() => {
  vi.useRealTimers();
});

function setup(prefs: BoardActivityPrefs = ALL_ON) {
  const client = new FakeDaemon();
  const select = vi.fn<(selection: Selection) => void>();
  const bots = [bot({ id: "b1", name: "New Bot", project_id: "p1" })];
  const hook = renderHook(
    (p: BoardActivityPrefs) => useBoardActivity({ client, bots, prefs: p, select, now: () => now }),
    { initialProps: prefs },
  );
  return { client, select, hook };
}

describe("useBoardActivity", () => {
  it("turns a bot's move and its finished turn on the card into one toast", () => {
    const { client, hook } = setup();
    act(() => {
      client.emit("bot_state", {
        type: "bot_state",
        bot_id: "b1",
        state: "working",
        reason: "prompt submitted",
        at: "",
        item_id: "P2-001",
        item_title: "Test",
      });
      client.emit("board_activity", { type: "board_activity", ...moved() });
      client.emit("bot_state", {
        type: "bot_state",
        bot_id: "b1",
        state: "ready",
        reason: "turn complete",
        at: "",
        item_id: "P2-001",
      });
    });
    const toasts = hook.result.current.toasts;
    expect(toasts).toHaveLength(1);
    expect(toasts[0]?.actorLabel).toBe("New Bot");
    expect(toasts[0]?.lines.map((l) => l.word)).toEqual(["Started", "Moved", "Finished"]);
  });

  it("skips the owner's own action on this app", () => {
    const { client, hook } = setup();
    act(() => {
      client.emit("board_activity", {
        type: "board_activity",
        ...moved({ actor: "user", from_this_client: true }),
      });
    });
    expect(hook.result.current.toasts).toEqual([]);
  });

  it("says nothing of a kind turned off in Settings", () => {
    const { client, hook } = setup({ ...ALL_ON, moved: false });
    act(() => {
      client.emit("board_activity", { type: "board_activity", ...moved() });
      client.emit("board_activity", {
        type: "board_activity",
        ...moved({ kind: "BOARD_ACTIVITY_KIND_MOVE_REFUSED", unmet: ["Not yet."] }),
      });
    });
    expect(hook.result.current.toasts.map((t) => t.lines[0]?.word)).toEqual(["Couldn't move"]);
    hook.rerender({ ...ALL_ON, refused: false });
    act(() => {
      client.emit("board_activity", {
        type: "board_activity",
        ...moved({ item_id: "P2-009", kind: "BOARD_ACTIVITY_KIND_MOVE_REFUSED" }),
      });
    });
    expect(hook.result.current.toasts).toHaveLength(1);
  });

  it("keeps at most three and lets them go after a while, unless held", () => {
    const { client, hook } = setup();
    act(() => {
      for (const id of ["A-1", "A-2", "A-3", "A-4"]) {
        client.emit("board_activity", { type: "board_activity", ...moved({ item_id: id }) });
      }
    });
    expect(hook.result.current.toasts.map((t) => t.itemId)).toEqual(["A-4", "A-3", "A-2"]);
    act(() => {
      hook.result.current.hold(true);
      now = ACTIVITY_TTL_MS + 1;
      vi.advanceTimersByTime(2000);
    });
    expect(hook.result.current.toasts).toHaveLength(3);
    act(() => {
      hook.result.current.hold(false);
      now += ACTIVITY_TTL_MS + 1;
      vi.advanceTimersByTime(2000);
    });
    expect(hook.result.current.toasts).toEqual([]);
  });

  it("opens the card's drawer on its project's Board", () => {
    const { client, select, hook } = setup();
    act(() => {
      client.emit("board_activity", { type: "board_activity", ...moved() });
    });
    const [toast] = hook.result.current.toasts;
    if (toast === undefined) {
      throw new Error("expected a toast");
    }
    act(() => {
      hook.result.current.open(toast);
    });
    expect(select).toHaveBeenCalledWith({
      kind: "project",
      projectId: "p1",
      tab: "board",
      item: "P2-001",
    });
    expect(hook.result.current.toasts).toEqual([]);
  });
});

describe("ActivityToasts", () => {
  const toast: ActivityToast = {
    id: 1,
    key: "k",
    actorLabel: "New Bot",
    projectId: "p1",
    itemId: "P2-001",
    itemTitle: "Test",
    lines: [{ id: 1, glyph: "→", word: "Moved", detail: "To do → Doing" }],
    updatedAt: 0,
  };

  it("is a polite live region whose toasts open their card or go on Escape", () => {
    const onOpen = vi.fn<(t: ActivityToast) => void>();
    const onDismiss = vi.fn<(id: number) => void>();
    render(
      <ActivityToasts toasts={[toast]} onOpen={onOpen} onDismiss={onDismiss} onHold={() => {}} />,
    );
    const region = screen.getByRole("region", { name: "Board activity" });
    expect(region.getAttribute("aria-live")).toBe("polite");
    const open = screen.getByRole("button", { name: /New Bot, P2-001 “Test”: Moved/ });
    expect(screen.getByText("Moved")).toBeTruthy();
    fireEvent.click(open);
    expect(onOpen).toHaveBeenCalledWith(toast);
    fireEvent.keyDown(open, { key: "Escape" });
    expect(onDismiss).toHaveBeenCalledWith(1);
    fireEvent.click(screen.getByRole("button", { name: "Dismiss: New Bot, P2-001" }));
    expect(onDismiss).toHaveBeenCalledTimes(2);
  });
});

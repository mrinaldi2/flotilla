import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FakeDaemon } from "../../test/fakeDaemon";
import { useOnCommandsChanged } from "./useOnCommandsChanged";

const push = (bot_id: string, project_id: string) =>
  ({ type: "commands_changed", bot_id, project_id, daemon_id: "d-mac" }) as const;

describe("useOnCommandsChanged", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("refreshes once a burst of pushes settles", () => {
    const client = new FakeDaemon();
    const refresh = vi.fn<() => Promise<void>>(() => Promise.resolve());
    renderHook(() => useOnCommandsChanged(client, true, refresh));
    act(() => {
      client.emit("commands_changed", push("b1", "p1"));
      vi.advanceTimersByTime(300);
      client.emit("commands_changed", push("b2", "p2"));
      vi.advanceTimersByTime(499);
    });
    expect(refresh).not.toHaveBeenCalled();
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(refresh).toHaveBeenCalledTimes(1);
  });

  it("skips other projects and bots, and listens only while enabled", () => {
    const client = new FakeDaemon();
    const refresh = vi.fn<() => Promise<void>>(() => Promise.resolve());
    const { rerender } = renderHook(
      ({ enabled }: { enabled: boolean }) =>
        useOnCommandsChanged(client, enabled, refresh, "p1", "b1"),
      { initialProps: { enabled: true } },
    );
    act(() => {
      client.emit("commands_changed", push("b1", "p2"));
      client.emit("commands_changed", push("b2", "p1"));
      vi.advanceTimersByTime(1000);
    });
    expect(refresh).not.toHaveBeenCalled();
    rerender({ enabled: false });
    act(() => {
      client.emit("commands_changed", push("b1", "p1"));
      vi.advanceTimersByTime(1000);
    });
    expect(refresh).not.toHaveBeenCalled();
  });
});

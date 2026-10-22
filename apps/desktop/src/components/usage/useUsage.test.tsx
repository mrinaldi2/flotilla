import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { UsageRange } from "../../protocol/usage";
import { DaemonError } from "../../protocol/connection";
import { usageDaemon } from "../../test/usageFixtures";
import { USAGE_REFETCH_MS, useUsage } from "./useUsage";

function reads(fake: ReturnType<typeof usageDaemon>) {
  return fake.requests.filter((r) => r.body.type === "usage_get").map((r) => r.body);
}

describe("useUsage", () => {
  it("reads the chosen range, and again when it changes", async () => {
    const fake = usageDaemon();
    const { result, rerender } = renderHook(
      ({ range }: { readonly range: UsageRange }) => useUsage(fake, true, range),
      { initialProps: { range: "7d" } },
    );
    await waitFor(() => expect(result.current.usage?.range).toBe("7d"));
    rerender({ range: "today" });
    await waitFor(() => expect(result.current.usage?.range).toBe("today"));
    expect(reads(fake)).toEqual([
      { type: "usage_get", range: "7d" },
      { type: "usage_get", range: "today" },
    ]);
  });

  it("reads nothing while disconnected", () => {
    const fake = usageDaemon();
    renderHook(() => useUsage(fake, false, "7d"));
    expect(reads(fake)).toEqual([]);
  });

  it("reads once, a moment after a burst of usage_updated pushes", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const fake = usageDaemon();
      const { result } = renderHook(() => useUsage(fake, true, "7d"));
      await waitFor(() => expect(result.current.usage).not.toBeNull());
      act(() => {
        fake.emit("usage_updated", { type: "usage_updated", project_id: "p1" });
        fake.emit("usage_updated", { type: "usage_updated", project_id: "p2" });
      });
      expect(reads(fake)).toHaveLength(1);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(USAGE_REFETCH_MS);
      });
      expect(reads(fake)).toHaveLength(2);
    } finally {
      vi.useRealTimers();
    }
  });

  it("keeps what it read and says why a later read failed", async () => {
    const fake = usageDaemon();
    const { result } = renderHook(() => useUsage(fake, true, "7d"));
    await waitFor(() => expect(result.current.usage).not.toBeNull());
    fake.onRequest("usage_get", () => {
      throw new DaemonError("internal", "the table is locked");
    });
    await act(async () => {
      await result.current.reload();
    });
    expect(result.current.error).toMatch(/the table is locked/);
    expect(result.current.usage?.range).toBe("7d");
  });
});

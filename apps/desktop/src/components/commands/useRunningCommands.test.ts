import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { commandsDaemon, commandsReply } from "../../test/commandFixtures";
import { useRunningCommands } from "./useRunningCommands";

describe("useRunningCommands", () => {
  it("adds up every computer's running commands", async () => {
    const client = commandsDaemon();
    const { result } = renderHook(() => useRunningCommands(client, true));
    await waitFor(() => {
      expect(result.current).toBe(3);
    });
    expect(client.requests[0]?.body).toEqual({
      type: "list_commands",
      status: "running",
      limit: 1,
    });
  });

  it("reads again when commands change", async () => {
    let running = 1;
    const client = commandsDaemon(() =>
      commandsReply({
        summary: [{ daemon_id: "d-mac", computer_name: "mac", running, failed_today: 0 }],
      }),
    );
    const { result } = renderHook(() => useRunningCommands(client, true));
    await waitFor(() => {
      expect(result.current).toBe(1);
    });
    running = 4;
    act(() => {
      client.emit("commands_changed", {
        type: "commands_changed",
        bot_id: "b1",
        project_id: "p1",
        daemon_id: "d-mac",
      });
    });
    await waitFor(() => {
      expect(result.current).toBe(4);
    });
  });

  it("is 0 while disabled or when the read fails", async () => {
    const client = commandsDaemon();
    const { result, rerender } = renderHook(
      ({ enabled }: { enabled: boolean }) => useRunningCommands(client, enabled),
      { initialProps: { enabled: false } },
    );
    expect(result.current).toBe(0);
    expect(client.requests).toHaveLength(0);

    client.onRequest("list_commands", () => {
      throw new Error("unknown request type");
    });
    rerender({ enabled: true });
    await waitFor(() => {
      expect(client.requests).toHaveLength(1);
    });
    expect(result.current).toBe(0);
  });
});

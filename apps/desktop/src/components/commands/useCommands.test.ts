import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { commandRow, commandsDaemon, commandsReply } from "../../test/commandFixtures";
import type { FakeDaemon } from "../../test/fakeDaemon";
import type { CommandFilters } from "./useCommands";
import { useCommands } from "./useCommands";

const ALL: CommandFilters = { projectId: null, botId: null, status: "all", search: "" };

function listed(client: FakeDaemon): readonly unknown[] {
  return client.requests.filter((r) => r.body.type === "list_commands").map((r) => r.body);
}

describe("useCommands", () => {
  it("lists every command, a page at first", async () => {
    const client = commandsDaemon();
    const { result } = renderHook(() => useCommands(client, true, ALL));
    await waitFor(() => {
      expect(result.current.loaded).toBe(true);
    });
    expect(result.current.rows).toHaveLength(6);
    expect(result.current.summary).toHaveLength(2);
    expect(listed(client)).toEqual([{ type: "list_commands", status: "all", limit: 50 }]);
  });

  it("sends the filters, and the search once typing settles", async () => {
    const client = commandsDaemon();
    const { rerender } = renderHook(
      ({ filters }: { filters: CommandFilters }) => useCommands(client, true, filters),
      { initialProps: { filters: ALL } },
    );
    rerender({ filters: { projectId: "p1", botId: "b2", status: "failed", search: "" } });
    await waitFor(() => {
      expect(listed(client)).toContainEqual({
        type: "list_commands",
        project_id: "p1",
        bot_id: "b2",
        status: "failed",
        limit: 50,
      });
    });
    rerender({ filters: { ...ALL, search: " play" } });
    rerender({ filters: { ...ALL, search: " playwright " } });
    await waitFor(() => {
      expect(listed(client).at(-1)).toEqual({
        type: "list_commands",
        status: "all",
        query: "playwright",
        limit: 50,
      });
    });
    expect(listed(client)).not.toContainEqual(expect.objectContaining({ query: "play" }));
  });

  it("reads again when the daemon says commands changed", async () => {
    let status: "running" | "done" = "running";
    const client = commandsDaemon(() => commandsReply({ commands: [commandRow({ status })] }));
    const { result } = renderHook(() => useCommands(client, true, ALL));
    await waitFor(() => {
      expect(result.current.rows[0]?.status).toBe("running");
    });
    status = "done";
    act(() => {
      client.emit("commands_changed", {
        type: "commands_changed",
        bot_id: "b1",
        project_id: "p1",
        daemon_id: "d-mac",
      });
    });
    await waitFor(() => {
      expect(result.current.rows[0]?.status).toBe("done");
    });
  });

  it("loads more after the last row, and rereads as many", async () => {
    const client = commandsDaemon((body) =>
      body.type === "list_commands" && body.before === "c1"
        ? commandsReply({ commands: [commandRow({ id: "c2" })] })
        : commandsReply({ commands: [commandRow()], next_before: "c1" }),
    );
    const { result } = renderHook(() => useCommands(client, true, ALL));
    await waitFor(() => {
      expect(result.current.nextBefore).toBe("c1");
    });
    await act(() => result.current.loadMore());
    expect(result.current.rows.map((r) => r.id)).toEqual(["c1", "c2"]);
    expect(result.current.nextBefore).toBeUndefined();
    expect(listed(client).at(-1)).toEqual({
      type: "list_commands",
      status: "all",
      limit: 50,
      before: "c1",
    });
    // Nothing after the last page: no request.
    const asked = listed(client).length;
    await act(() => result.current.loadMore());
    expect(listed(client)).toHaveLength(asked);
    await act(() => result.current.refresh());
    expect(listed(client).at(-1)).toEqual({ type: "list_commands", status: "all", limit: 51 });
  });

  it("keeps the rows when the next page fails", async () => {
    const client = commandsDaemon((body) => {
      if (body.type === "list_commands" && body.before !== undefined) {
        throw new Error("peer is offline");
      }
      return commandsReply({ commands: [commandRow()], next_before: "c1" });
    });
    const { result } = renderHook(() => useCommands(client, true, ALL));
    await waitFor(() => {
      expect(result.current.nextBefore).toBe("c1");
    });
    await act(() => result.current.loadMore());
    expect(result.current.error).toBe("peer is offline");
    expect(result.current.rows).toHaveLength(1);
  });

  it("keeps the daemon's words when a read fails", async () => {
    const client = commandsDaemon(() => {
      throw new Error("status must be running, failed or all");
    });
    const { result } = renderHook(() => useCommands(client, true, ALL));
    await waitFor(() => {
      expect(result.current.error).toBe("status must be running, failed or all");
    });
    expect(result.current.loaded).toBe(true);
  });

  it("waits for the connection", () => {
    const client = commandsDaemon();
    renderHook(() => useCommands(client, false, ALL));
    expect(listed(client)).toHaveLength(0);
  });
});

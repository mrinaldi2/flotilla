import { renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { commandsDaemon } from "../test/commandFixtures";
import { USAGE_V1 } from "../protocol/usage";
import { useRailPages } from "./useRailPages";

describe("useRailPages", () => {
  it("offers Commands, with what runs, and Usage to a service that serves them", async () => {
    const client = commandsDaemon();
    client.capabilities = [...client.capabilities, USAGE_V1];
    const { result } = renderHook(() => useRailPages(client, true));
    await waitFor(() => {
      expect(result.current).toEqual({ commands: true, commandsRunning: 3, usage: true });
    });
  });

  it("offers neither while disconnected, and asks nothing", () => {
    const client = commandsDaemon();
    const { result } = renderHook(() => useRailPages(client, false));
    expect(result.current).toEqual({ commands: false, commandsRunning: 0, usage: false });
    expect(client.requests).toHaveLength(0);
  });
});

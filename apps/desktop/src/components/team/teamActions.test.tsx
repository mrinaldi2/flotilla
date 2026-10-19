import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { NotifyLevel } from "../../protocol/entities";
import { FakeDaemon } from "../../test/fakeDaemon";
import { bot } from "../../test/fixtures";
import { TEAM_STATUS, teamDaemon } from "../../test/teamFixtures";
import { useTeamStatus, wakeOrSleep } from "./teamActions";

type Toast = (level: NotifyLevel, title: string, body: string) => void;

describe("the team's running count", () => {
  it("is read on connect and kept by team_updated", async () => {
    const fake = teamDaemon();
    const { result } = renderHook(() => useTeamStatus(fake, true));
    await waitFor(() => {
      expect(result.current).toEqual(TEAM_STATUS);
    });
    const next = { ...TEAM_STATUS, running: 6, queue: [] };
    act(() => {
      fake.emit("team_updated", { type: "team_updated", team: next });
    });
    expect(result.current).toEqual(next);
  });

  it("isn't asked of a service before 0.23", () => {
    const fake = new FakeDaemon();
    const { result } = renderHook(() => useTeamStatus(fake, true));
    expect(result.current).toBeNull();
    expect(fake.requests).toEqual([]);
  });
});

describe("waking a bot and sending it to sleep", () => {
  it("sends the request and says what happens next", async () => {
    const fake = teamDaemon();
    const onToast = vi.fn<Toast>();
    await wakeOrSleep(fake, bot(), "wake", onToast);
    await wakeOrSleep(fake, bot(), "sleep", onToast);
    expect(fake.requests.map((r) => r.body)).toEqual([
      { type: "wake_bot", bot_id: "b1" },
      { type: "sleep_bot", bot_id: "b1" },
    ]);
    expect(onToast).toHaveBeenNthCalledWith(1, "info", "Waking alice", expect.any(String));
    expect(onToast).toHaveBeenNthCalledWith(2, "info", "alice will sleep", expect.any(String));
  });

  it("reports a refusal", async () => {
    const fake = teamDaemon().onRequest("sleep_bot", () => {
      throw new Error("alice is always on: make it on demand first");
    });
    const onToast = vi.fn<Toast>();
    await wakeOrSleep(fake, bot(), "sleep", onToast);
    expect(onToast).toHaveBeenCalledWith(
      "error",
      "Couldn't put alice to sleep",
      "alice is always on: make it on demand first",
    );
  });
});

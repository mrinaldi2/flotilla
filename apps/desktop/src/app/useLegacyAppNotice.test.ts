import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { stubLocalStorage } from "../test/spies";
import { legacyAppMessage, useLegacyAppNotice } from "./useLegacyAppNotice";
import type { AddToast } from "./useToasts";

const invoke = vi.hoisted(() => vi.fn<(command: string) => Promise<unknown>>());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("../tauri", () => ({ isTauri: () => true }));

const APP = "/Applications/The Hermes.app";

afterEach(() => {
  invoke.mockReset();
});

function mount(addToast: AddToast, now: number): void {
  const clock = (): number => now;
  renderHook(() => {
    useLegacyAppNotice(addToast, clock);
  });
}

describe("useLegacyAppNotice", () => {
  it("tells the owner the old app can go, naming it and the permissions", async () => {
    stubLocalStorage({});
    invoke.mockResolvedValue(APP);
    const addToast = vi.fn<AddToast>();
    mount(addToast, 1_000);
    await waitFor(() => {
      expect(addToast).toHaveBeenCalledTimes(1);
    });
    expect(addToast).toHaveBeenCalledWith(
      "info",
      "The Hermes is still installed",
      legacyAppMessage(APP),
      { sticky: true },
    );
    expect(legacyAppMessage(APP)).toContain(APP);
    expect(legacyAppMessage(APP)).toContain("microphone");
  });

  it("stays quiet when the old app is gone, or was mentioned today", async () => {
    stubLocalStorage({ "flotilla.legacy-app-notice": "1000" });
    invoke.mockResolvedValue(APP);
    const addToast = vi.fn<AddToast>();
    mount(addToast, 2_000);
    invoke.mockResolvedValue(null);
    mount(addToast, 1_000 + 2 * 24 * 60 * 60 * 1000);
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledTimes(2);
    });
    expect(addToast).not.toHaveBeenCalled();
  });
});

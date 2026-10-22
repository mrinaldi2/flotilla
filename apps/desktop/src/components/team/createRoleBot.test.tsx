import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { AddToast } from "../../app/useToasts";
import { DaemonError } from "../../protocol/connection";
import { Role } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { snapshot } from "../../test/boardFixtures";
import { FakeDaemon } from "../../test/fakeDaemon";
import { bot } from "../../test/fixtures";
import { createRoleBot, roleOfToken, useCreateRoleBot } from "./createRoleBot";

function daemon(): FakeDaemon {
  return new FakeDaemon()
    .onRequest("create_bot", () => ({
      type: "bot",
      req_id: "1",
      bot: bot({ id: "b9", name: "Architect" }),
    }))
    .onBoard("roleSet", () => ({ case: "board", value: snapshot() }));
}

describe("a role's token", () => {
  it("names the role, whatever its case", () => {
    expect(roleOfToken("architect")).toBe(Role.ARCHITECT);
    expect(roleOfToken("reviewer.ux")).toBe(Role.REVIEWER_UX);
    expect(roleOfToken(" Dev ")).toBe(Role.DEV);
    expect(roleOfToken("astronaut")).toBeUndefined();
    expect(roleOfToken(undefined)).toBeUndefined();
  });
});

describe("creating a bot with a role", () => {
  it("creates a bot named for the role, then gives it the role", async () => {
    const fake = daemon();
    const made = await createRoleBot(fake, "p1", Role.ARCHITECT);
    expect(fake.requests.map((r) => r.body)).toEqual([
      { type: "create_bot", project_id: "p1", name: "Architect" },
    ]);
    expect(fake.boardCalls).toMatchObject([
      { case: "roleSet", value: { projectId: "p1", bot: "b9", role: Role.ARCHITECT } },
    ]);
    expect(made.bot.id).toBe("b9");
    expect(made.roleError).toBeNull();
  });

  it("says so, and hands the bot on, when it worked", async () => {
    const addToast = vi.fn<AddToast>();
    const onCreated = vi.fn<(b: unknown, s: unknown) => void>();
    const { result } = renderHook(() =>
      useCreateRoleBot(daemon(), "p1", addToast, (b, s) => onCreated(b, s)),
    );
    act(() => result.current.create(Role.ARCHITECT));
    expect(result.current.creating).toBe(Role.ARCHITECT);
    await waitFor(() => {
      expect(result.current.creating).toBeNull();
    });
    expect(addToast).toHaveBeenCalledWith(
      "info",
      "Architect holds Architect now",
      "Its waiting cards go to it.",
    );
    expect(onCreated).toHaveBeenCalledWith(expect.objectContaining({ id: "b9" }), snapshot());
  });

  it("says the bot exists when the role was refused", async () => {
    const fake = daemon().onBoard("roleSet", () => {
      throw new DaemonError("forbidden", "only the owner sets roles");
    });
    const addToast = vi.fn<AddToast>();
    const { result } = renderHook(() => useCreateRoleBot(fake, "p1", addToast));
    act(() => result.current.create(Role.PRODUCT));
    await waitFor(() => {
      expect(addToast).toHaveBeenCalledWith(
        "error",
        "Created Architect, but couldn't make it the Product",
        "only the owner sets roles",
      );
    });
  });

  it("says why no bot was created", async () => {
    const fake = new FakeDaemon().onRequest("create_bot", () => {
      throw new Error("the project is gone");
    });
    const addToast = vi.fn<AddToast>();
    const { result } = renderHook(() => useCreateRoleBot(fake, "p1", addToast));
    act(() => result.current.create(Role.TESTER));
    await waitFor(() => {
      expect(addToast).toHaveBeenCalledWith(
        "error",
        "Couldn't create a Tester bot",
        "the project is gone",
      );
    });
    expect(fake.boardCalls).toEqual([]);
  });
});

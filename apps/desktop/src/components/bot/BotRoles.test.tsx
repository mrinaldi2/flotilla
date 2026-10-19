import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { DaemonError } from "../../protocol/connection";
import { ProjectRoleSchema, Role } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { ProjectRole } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { applyRoleSet, snapshot } from "../../test/boardFixtures";
import { TEAM_ON_DEMAND_V1 } from "../../protocol/team";
import { FakeDaemon } from "../../test/fakeDaemon";
import { bot } from "../../test/fixtures";
import BotRoles from "./BotRoles";

function role(r: Role, botId = "b1", machine?: string): ProjectRole {
  return create(ProjectRoleSchema, { projectId: "p1", role: r, botId, machine });
}

function board(roles: readonly ProjectRole[], canRule: boolean): FakeDaemon {
  return new FakeDaemon().onBoard("boardGet", () => ({
    case: "board",
    value: { ...snapshot(), roles: [...roles], canRule },
  }));
}

describe("BotRoles", () => {
  it("lists only this bot's roles, a tester with its machine", async () => {
    const fake = board(
      [role(Role.DEV), role(Role.TESTER, "b1", "imac"), role(Role.LEAD, "b2")],
      false,
    );
    render(<BotRoles client={fake} bot={bot()} connected />);
    const list = await screen.findByRole("list", { name: "alice's board roles" });
    expect(within(list).getByText("Developer")).toBeInTheDocument();
    expect(within(list).getByText("Tester · imac")).toBeInTheDocument();
    expect(within(list).queryByText("Lead")).toBeNull();
    // Not the owner on the board's home: no controls.
    expect(screen.queryByRole("button", { name: /Remove/ })).toBeNull();
    expect(screen.queryByRole("button", { name: "Add role" })).toBeNull();
  });

  it("lets the owner add reviewer.ce and take a role away", async () => {
    const user = userEvent.setup();
    let roles = [role(Role.DEV)];
    const fake = board(roles, true).onBoard("roleSet", (call) => {
      roles = applyRoleSet(roles, call);
      return { case: "board", value: { ...snapshot(), roles, canRule: true } };
    });
    render(<BotRoles client={fake} bot={bot()} connected />);
    await screen.findByText("Developer", { selector: "span" });
    await user.selectOptions(
      screen.getByRole("combobox", { name: "Role to add" }),
      "Reviewer · security (ce)",
    );
    await user.click(screen.getByRole("button", { name: "Add role" }));
    expect(
      await screen.findByText("Reviewer · security (ce)", { selector: "span" }),
    ).toBeInTheDocument();
    expect(fake.boardCalls.find((c) => c.case === "roleSet")?.value).toMatchObject({
      projectId: "p1",
      bot: "b1",
      role: Role.REVIEWER_CE,
    });
    await user.click(screen.getByRole("button", { name: "Remove Developer" }));
    expect(fake.boardCalls.at(-1)?.value).toMatchObject({ role: Role.DEV, remove: true });
    expect(await screen.findByRole("list")).not.toHaveTextContent("Developer");
  });

  it("asks a tester's machine first, then sends it", async () => {
    const user = userEvent.setup();
    const fake = board([], true).onBoard("roleSet", () => ({
      case: "board",
      value: { ...snapshot(), roles: [role(Role.TESTER, "b1", "ios")], canRule: true },
    }));
    render(<BotRoles client={fake} bot={bot()} connected />);
    await screen.findByText("No role on the board.");
    await user.selectOptions(screen.getByRole("combobox", { name: "Role to add" }), "Tester");
    await user.click(screen.getByRole("button", { name: "Add role" }));
    expect(screen.getByRole("alert")).toHaveTextContent(/which machine/);
    expect(fake.boardCalls.some((c) => c.case === "roleSet")).toBe(false);
    await user.type(screen.getByRole("textbox", { name: "Tester machine" }), " ios ");
    await user.click(screen.getByRole("button", { name: "Add role" }));
    expect(fake.boardCalls.at(-1)?.value).toMatchObject({ role: Role.TESTER, machine: "ios" });
    expect(await screen.findByText("Tester · ios", { selector: "span" })).toBeInTheDocument();
  });

  it("shows the daemon's refusal", async () => {
    const user = userEvent.setup();
    const fake = board([], true).onBoard("roleSet", () => {
      throw new DaemonError(
        "invalid_request",
        "only the owner, from the app or a paired device, assigns reviewer.ce",
      );
    });
    render(<BotRoles client={fake} bot={bot()} connected />);
    await screen.findByText("No role on the board.");
    await user.click(screen.getByRole("button", { name: "Add role" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/paired device/);
  });

  it("offers Product and Architect only on a 0.23 service", async () => {
    const old = board([], true);
    const { unmount } = render(<BotRoles client={old} bot={bot()} connected />);
    const picker = await screen.findByRole("combobox", { name: "Role to add" });
    expect(within(picker).queryByRole("option", { name: "Product" })).toBeNull();
    unmount();
    const team = board([role(Role.PRODUCT)], true);
    team.capabilities = [...team.capabilities, TEAM_ON_DEMAND_V1];
    render(<BotRoles client={team} bot={bot()} connected />);
    const list = await screen.findByRole("list", { name: "alice's board roles" });
    expect(within(list).getByText("Product")).toBeInTheDocument();
    const offered = within(screen.getByRole("combobox", { name: "Role to add" }));
    expect(offered.getByRole("option", { name: "Product" })).toBeDisabled();
    expect(offered.getByRole("option", { name: "Architect" })).toBeEnabled();
  });
});

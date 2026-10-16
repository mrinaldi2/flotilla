import type { Story } from "@ladle/react";
import { create } from "@bufbuild/protobuf";
import { useState } from "react";
import type { ReactElement } from "react";
import { DaemonError } from "../../protocol/connection";
import { ProjectRoleSchema, Role } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { ProjectRole } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { applyRoleSet, snapshot } from "../../test/boardFixtures";
import { FakeDaemon } from "../../test/fakeDaemon";
import { bot } from "../../test/fixtures";
import BotRoles from "./BotRoles";

const TESTER = bot({ id: "b1", name: "Tester Win" });

function role(r: Role, machine?: string): ProjectRole {
  return create(ProjectRoleSchema, { projectId: "p1", role: r, botId: "b1", machine });
}

/** A fake board that keeps role changes, or refuses every one of them. */
function Frame(props: {
  readonly roles: readonly ProjectRole[];
  readonly canRule: boolean;
  readonly refuse?: string;
}): ReactElement {
  const [client] = useState(() => {
    let roles = [...props.roles];
    return new FakeDaemon()
      .onBoard("boardGet", () => ({
        case: "board",
        value: { ...snapshot(), roles, canRule: props.canRule },
      }))
      .onBoard("roleSet", (call) => {
        if (props.refuse !== undefined) {
          throw new DaemonError("invalid_request", props.refuse);
        }
        roles = applyRoleSet(roles, call);
        return { case: "board", value: { ...snapshot(), roles, canRule: props.canRule } };
      });
  });
  return (
    <div className="main" style={{ padding: 16, maxWidth: 360 }}>
      <BotRoles client={client} bot={TESTER} connected />
    </div>
  );
}

/** The owner on the board's home: chips with ✕, and the picker. */
export const Owner: Story = () => (
  <Frame roles={[role(Role.DEV), role(Role.TESTER, "win-pc")]} canRule />
);

/** Anyone else: the roles, and why there are no controls. */
export const ReadOnly: Story = () => (
  <Frame roles={[role(Role.DEV), role(Role.TESTER, "win-pc")]} canRule={false} />
);

/** No role yet; the service refuses the change (H-326 AC0). */
export const Refused: Story = () => (
  <Frame
    roles={[]}
    canRule
    refuse="only the owner, from the app or a paired device, assigns reviewer.ce"
  />
);

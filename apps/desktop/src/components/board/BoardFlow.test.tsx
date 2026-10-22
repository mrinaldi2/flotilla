import { create } from "@bufbuild/protobuf";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ProjectRoleSchema, Role } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { BOARD_FLOW_V1 } from "../../protocol/team";
import { flowColumns, flowRoles } from "../../test/boardFlowFixtures";
import { boardsSnapshot, mainColumns, RECIPES_ID } from "../../test/boardsFixtures";
import { BoardWindow, boardsClient } from "../../test/boardStory";
import { bot } from "../../test/fixtures";

function flowClient(capable: boolean) {
  const columns = [...mainColumns(), ...flowColumns()];
  const client = boardsClient(boardsSnapshot({ columns, roles: flowRoles() }))
    .onRequest("create_bot", () => ({
      type: "bot",
      req_id: "1",
      bot: bot({ id: "b9", name: "Product" }),
    }))
    .onBoard("roleSet", () => ({
      case: "board",
      value: boardsSnapshot({
        columns,
        roles: [
          ...flowRoles(),
          create(ProjectRoleSchema, { projectId: "p1", role: Role.PRODUCT, botId: "b9" }),
        ],
      }),
    }));
  if (capable) {
    client.capabilities = [...client.capabilities, BOARD_FLOW_V1];
  }
  return client;
}

async function openColumns(client: ReturnType<typeof flowClient>): Promise<void> {
  const user = userEvent.setup();
  render(<BoardWindow client={client} openBoard={RECIPES_ID} />);
  await screen.findByText("Shopping list by aisle");
  await user.click(screen.getByRole("button", { name: "Board" }));
  await user.click(screen.getByRole("tab", { name: "Columns" }));
}

describe("the board runs the SDLC", () => {
  it("creates a bot for a role nobody holds, and the notice goes", async () => {
    const user = userEvent.setup();
    const client = flowClient(true);
    await openColumns(client);
    expect(screen.getByText("Nobody holds Product")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Create a Product bot" }));
    await waitFor(() => {
      expect(screen.queryByText("Nobody holds Product")).toBeNull();
    });
    expect(client.requests.map((r) => r.body)).toContainEqual({
      type: "create_bot",
      project_id: "p1",
      name: "Product",
    });
    expect(client.boardCalls.find((c) => c.case === "roleSet")?.value).toMatchObject({
      bot: "b9",
      role: Role.PRODUCT,
    });
    expect(screen.getByRole("combobox", { name: "On entry, To do assigns" })).toHaveValue(
      String(Role.PRODUCT),
    );
  });

  it("offers no On entry without board_flow_v1", async () => {
    await openColumns(flowClient(false));
    expect(screen.queryByRole("combobox", { name: /On entry/ })).toBeNull();
  });
});

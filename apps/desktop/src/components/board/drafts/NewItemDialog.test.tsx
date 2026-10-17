import { create } from "@bufbuild/protobuf";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { BoardSchema, ItemType } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { EditResultSchema } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { draftItem } from "../../../test/boardsFixtures";
import { FakeDaemon } from "../../../test/fakeDaemon";
import { PLAN_BOARD, planClient, planNodes } from "../../../test/planFixtures";
import NewItemDialog from "./NewItemDialog";

function dialog(client: FakeDaemon, withPlan: boolean) {
  client.onBoard("itemCreate", () => ({
    case: "edited",
    value: create(EditResultSchema, { outcome: { case: "done", value: draftItem() } }),
  }));
  const onCreated = vi.fn<(id: string) => void>();
  render(
    <NewItemDialog
      api={client}
      projectId="p1"
      board={create(BoardSchema, { id: PLAN_BOARD, name: "Combat readability" })}
      planNodes={withPlan ? planNodes() : undefined}
      onCreated={onCreated}
      onCancel={() => {}}
    />,
  );
  return onCreated;
}

function created(client: FakeDaemon) {
  return client.boardCalls.flatMap((c) => (c.case === "itemCreate" ? [c.value] : []));
}

describe("+ New item", () => {
  it("offers Task, Design, Bug and Spike, Task first, and points to the Plan", async () => {
    const user = userEvent.setup();
    const client = planClient();
    const onCreated = dialog(client, false);
    const types = screen.getAllByRole("radio").map((r) => r.closest("label")?.textContent ?? "");
    expect(types.map((t) => t.split(".")[0])).toEqual([
      "■ TaskBuild something",
      "✎ DesignMockups or a spec",
      "✱ BugSomething broken",
      "◎ SpikeA timeboxed question that ends in a document",
    ]);
    expect(screen.getByRole("radio", { name: /Task/ })).toBeChecked();
    expect(screen.getByText(/Features and epics are created in the Plan/)).toBeVisible();
    expect(screen.queryByRole("combobox", { name: /Belongs to/ })).not.toBeInTheDocument();
    await user.type(
      screen.getByRole("textbox", { name: "Title" }),
      "Hitstop ignores UI time scale",
    );
    await user.click(screen.getByRole("button", { name: "Save as draft" }));
    expect(created(client)).toMatchObject([{ type: ItemType.TASK, planNodeId: undefined }]);
    expect(onCreated).toHaveBeenCalledWith("RB-20");
  });

  it("puts a card on an initiative's board under a node of its plan", async () => {
    const user = userEvent.setup();
    const client = planClient();
    dialog(client, true);
    const belongs = screen.getByRole("combobox", { name: /Belongs to/ });
    expect(belongs).toHaveValue("PL-6");
    await user.click(screen.getByRole("radio", { name: /Spike/ }));
    expect(belongs).toHaveValue("PL-2");
    await user.selectOptions(belongs, "PL-8");
    await user.type(screen.getByRole("textbox", { name: "Title" }), "Off-screen cues");
    await user.click(screen.getByRole("button", { name: "Save as draft" }));
    expect(created(client)).toMatchObject([
      { type: ItemType.SPIKE, planNodeId: "PL-8", draft: true },
    ]);
  });

  it("keeps the older types for a service before 0.21", () => {
    dialog(new FakeDaemon(), false);
    expect(screen.getByRole("radio", { name: /Feature/ })).toBeChecked();
    expect(screen.queryByText(/created in the Plan/)).not.toBeInTheDocument();
  });
});

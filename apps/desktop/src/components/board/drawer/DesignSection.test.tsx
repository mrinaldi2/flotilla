import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { LinkKind } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { EditResultSchema } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { DRAWER_NOW } from "../../../test/drawerFixtures";
import { bot } from "../../../test/fixtures";
import { initiativeColumns } from "../../../test/planFixtures";
import { designCard, openTasks, researchClient } from "../../../test/researchFixtures";
import { documentsLine } from "./DesignSection";
import ItemDrawer from "./ItemDrawer";

const BOTS = [bot({ id: "b1", name: "Designer" }), bot({ id: "b2", name: "Gameplay Dev" })];

function drawer(
  itemId: string,
  client = researchClient(),
  onOpenItem = vi.fn<(id: string) => void>(),
) {
  render(
    <ItemDrawer
      api={client}
      itemId={itemId}
      columns={initiativeColumns()}
      bots={BOTS}
      canComment
      onClose={() => {}}
      now={() => DRAWER_NOW}
      onOpenItem={onOpenItem}
      designTasks={openTasks()}
    />,
  );
  return { client, onOpenItem };
}

describe("a task held in Research", () => {
  it("says it can't move to Doing yet, with the guard's code", async () => {
    drawer("CR-28");
    const held = await screen.findByRole("alert");
    expect(held).toHaveTextContent("Can't move to Doing yet");
    expect(held).toHaveTextContent("Write what you found in the code under Code research");
    expect(held).toHaveTextContent("start.research");
    expect(screen.queryByRole("region", { name: "Next step" })).not.toBeInTheDocument();
    const stepper = screen.getByRole("list", { name: "Where it stands" });
    expect(within(stepper).getByRole("listitem", { current: "step" })).toHaveTextContent(
      "Research",
    );
  });

  it("lists the design it builds with its documents, and opens it", async () => {
    const user = userEvent.setup();
    const { onOpenItem } = drawer("CR-28");
    const design = await screen.findByRole("region", { name: "Design" });
    expect(await within(design).findByText(/Hitstop feel and timings · Done/)).toBeVisible();
    expect(within(design).getByText("design.md · timing-curve.html · 2 more")).toBeVisible();
    await user.click(within(design).getByRole("button", { name: "Read CR-19" }));
    expect(onOpenItem).toHaveBeenCalledWith("CR-19");
  });

  it("words a design card's documents", () => {
    expect(documentsLine(null)).toBe("No documents yet");
    expect(documentsLine(designCard())).toBe("design.md · timing-curve.html · 2 more");
  });
});

describe("a done Design card", () => {
  it("attaches to a task on its board", async () => {
    const user = userEvent.setup();
    const client = researchClient().onBoard("itemLink", () => ({
      case: "edited",
      value: create(EditResultSchema, { outcome: { case: "done", value: {} } }),
    }));
    drawer("CR-19", client);
    await user.click(await screen.findByRole("button", { name: "Attach design to a task…" }));
    await user.selectOptions(
      screen.getByRole("combobox", { name: "The task that builds it" }),
      "CR-28",
    );
    await user.click(screen.getByRole("button", { name: "Attach" }));
    const links = client.boardCalls.flatMap((c) => (c.case === "itemLink" ? [c.value] : []));
    expect(links).toEqual([{ id: "CR-28", kind: LinkKind.DESIGN, ref: "CR-19" }]);
    expect(await screen.findByRole("status")).toHaveTextContent("Attached to CR-28");
  });

  it("says why a link was refused", async () => {
    const user = userEvent.setup();
    const client = researchClient().onBoard("itemLink", () => ({
      case: "edited",
      value: create(EditResultSchema, {
        outcome: {
          case: "refused",
          value: { unmet: [{ code: "link.design", text: "Only a done design attaches." }] },
        },
      }),
    }));
    drawer("CR-19", client);
    await user.click(await screen.findByRole("button", { name: "Attach design to a task…" }));
    await user.click(screen.getByRole("button", { name: "Attach" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Only a done design attaches.");
  });
});

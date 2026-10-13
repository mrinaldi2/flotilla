import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import {
  EditResultSchema,
  MoveResultSchema,
} from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { unmet } from "../../../test/boardFixtures";
import { draftDetail, draftItem, recipeColumns } from "../../../test/boardsFixtures";
import { boardsClient, STORY_BOTS } from "../../../test/boardStory";
import * as fx from "../../../test/fixtures";
import ItemDrawer from "../drawer/ItemDrawer";

function renderDraft(client = boardsClient(), onDraftDone = vi.fn<(m: string) => void>()) {
  render(
    <ItemDrawer
      api={client}
      client={client}
      itemId="RB-20"
      version={6n}
      columns={recipeColumns()}
      bots={STORY_BOTS}
      canComment
      leadId="lead"
      onClose={vi.fn<() => void>()}
      onDraftDone={onDraftDone}
    />,
  );
  return { client, onDraftDone };
}

const edited = (outcome: "done" | "refused") =>
  create(EditResultSchema, {
    outcome:
      outcome === "done"
        ? { case: "done", value: draftItem() }
        : {
            case: "refused",
            value: { unmet: [unmet("draft.live", "RB-20 is not a draft any more.")] },
          },
  });

describe("a draft in the drawer", () => {
  it("is headed as a draft, with its inline screenshot and a placeholder for a removed one", async () => {
    renderDraft();
    expect(await screen.findByRole("complementary", { name: "Draft RB-20" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open the error full size" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "first try: removed" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Move to/ })).toBeNull();
  });

  it("reviews the changes since the owner looked, marks them seen, and undoes one", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("itemDraftUndo", () => ({
      case: "edited",
      value: edited("done"),
    }));
    renderDraft(client);
    expect(await screen.findByText("3 changes by Team Lead since you looked")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Review" }));
    const review = screen.getByRole("region", { name: "Changes since you looked" });
    const seen = client.boardCalls.find(
      (c) => c.case === "itemDraftChanges" && c.value.markSeen === true,
    );
    expect(seen?.case === "itemDraftChanges" && seen.value).toMatchObject({
      markSeen: true,
      since: 4n,
    });
    await user.click(within(review).getByRole("button", { name: /^Undo: Title/ }));
    const undo = client.boardCalls.find((c) => c.case === "itemDraftUndo");
    expect(undo?.case === "itemDraftUndo" && undo.value).toMatchObject({
      id: "RB-20",
      revision: 3n,
      expectedVersion: 6n,
    });
    expect(await within(review).findByText("Undone")).toBeInTheDocument();
  });

  it("shows a refused undo in words", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("itemDraftUndo", () => ({
      case: "edited",
      value: edited("refused"),
    }));
    renderDraft(client);
    await user.click(await screen.findByRole("button", { name: "Review" }));
    await user.click(screen.getByRole("button", { name: /^Undo: Title/ }));
    expect(await screen.findByText("RB-20 is not a draft any more.")).toBeInTheDocument();
  });

  it("lists the column's unmet rules when finalizing is refused, and reports a finalize", async () => {
    const user = userEvent.setup();
    let refuse = true;
    const client = boardsClient().onBoard("itemFinalize", () => ({
      case: "moved",
      value: create(MoveResultSchema, {
        outcome: refuse
          ? {
              case: "refused",
              value: { unmet: [unmet("rule.assignee_set", "Doing needs an assignee.")] },
            }
          : { case: "done", value: draftItem({ columnKey: "rb-doing" }) },
      }),
    }));
    const { onDraftDone } = renderDraft(client);
    await screen.findByRole("complementary", { name: "Draft RB-20" });
    await user.selectOptions(screen.getByRole("combobox", { name: "Column" }), "rb-doing");
    await user.click(screen.getByRole("button", { name: "Finalize" }));
    const refused = await screen.findByRole("alert");
    expect(refused).toHaveTextContent("Can't finalize into Doing:");
    expect(refused).toHaveTextContent("Doing needs an assignee.");
    refuse = false;
    await user.click(screen.getByRole("button", { name: "Finalize" }));
    await vi.waitFor(() =>
      expect(onDraftDone).toHaveBeenCalledWith("RB-20 is on the board, in Doing."),
    );
  });

  it("asks why before discarding", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("itemDiscard", () => ({
      case: "edited",
      value: edited("done"),
    }));
    const { onDraftDone } = renderDraft(client);
    await screen.findByRole("complementary", { name: "Draft RB-20" });
    await user.click(screen.getByRole("button", { name: "Discard" }));
    const dialog = screen.getByRole("dialog", { name: "Discard the draft RB-20" });
    await user.type(within(dialog).getByRole("textbox", { name: "Why" }), "Out of scope");
    await user.click(within(dialog).getByRole("button", { name: "Discard" }));
    const call = client.boardCalls.find((c) => c.case === "itemDiscard");
    expect(call?.case === "itemDiscard" && call.value).toMatchObject({ reason: "Out of scope" });
    await vi.waitFor(() => expect(onDraftDone).toHaveBeenCalledWith("RB-20 was discarded."));
  });

  it("invites the bot picked, docks its chat and sends every message with the item", async () => {
    const user = userEvent.setup();
    const client = boardsClient()
      .onBoard("itemInvite", () => ({ case: "item", value: draftDetail() }))
      .onRequest("send_user_message", () => ({
        type: "message",
        req_id: "1",
        message: fx.message(),
      }));
    renderDraft(client);
    await user.click(await screen.findByRole("button", { name: "Discuss with a bot" }));
    const menu = screen.getByRole("menu", { name: "Discuss the draft with" });
    const items = within(menu).getAllByRole("menuitem");
    expect(items[0]).toHaveTextContent("Team Lead");
    await user.click(within(menu).getByRole("menuitem", { name: /Tester/ }));
    const invite = client.boardCalls.find((c) => c.case === "itemInvite");
    expect(invite?.case === "itemInvite" && invite.value).toMatchObject({ id: "RB-20", bot: "b2" });
    const chat = await screen.findByRole("complementary", { name: "Tester about RB-20" });
    await user.type(within(chat).getByRole("textbox"), "Look at the screenshot{Enter}");
    await vi.waitFor(() =>
      expect(client.requests.map((r) => r.body)).toContainEqual({
        type: "send_user_message",
        to_bot_id: "b2",
        body: "Look at the screenshot",
        item_id: "RB-20",
      }),
    );
  });
});

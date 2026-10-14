import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ItemDetail } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { draftDetail, recipeColumns } from "../../../test/boardsFixtures";
import { boardsClient, STORY_BOTS } from "../../../test/boardStory";
import ItemDrawer from "../drawer/ItemDrawer";

/** The draft once no bot is invited to it. */
function nobodyInvited(): ItemDetail {
  const detail = draftDetail();
  detail.invited = [];
  return detail;
}

function Drawer(props: {
  readonly client: ReturnType<typeof boardsClient>;
  readonly invited?: readonly string[];
  readonly discuss?: string;
}) {
  return (
    <ItemDrawer
      api={props.client}
      client={props.client}
      itemId="RB-20"
      version={6n}
      invited={props.invited}
      columns={recipeColumns()}
      bots={STORY_BOTS}
      canComment
      leadId="lead"
      onClose={vi.fn<() => void>()}
      initialDiscuss={props.discuss}
    />
  );
}

describe("the bots discussing a draft", () => {
  it("lists them, and removes one only once the owner confirms", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("itemInvite", () => ({
      case: "item",
      value: nobodyInvited(),
    }));
    render(<Drawer client={client} />);
    const list = await screen.findByRole("region", { name: "Discussing the draft with" });
    expect(list).toHaveTextContent("Team Lead");
    const remove = within(list).getByRole("button", {
      name: "Remove Team Lead from the discussion",
    });

    await user.click(remove);
    const asked = screen.getByRole("dialog", { name: "Remove Team Lead from RB-20?" });
    expect(asked).toHaveTextContent("can no longer see or change the draft");
    await user.click(within(asked).getByRole("button", { name: "Keep" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(client.boardCalls.some((c) => c.case === "itemInvite")).toBe(false);

    await user.click(remove);
    const dialog = screen.getByRole("dialog", { name: "Remove Team Lead from RB-20?" });
    await user.click(within(dialog).getByRole("button", { name: "Remove" }));
    const call = client.boardCalls.find((c) => c.case === "itemInvite");
    expect(call?.case === "itemInvite" && call.value).toMatchObject({
      id: "RB-20",
      bot: "lead",
      remove: true,
    });
    await vi.waitFor(() =>
      expect(screen.queryByRole("region", { name: "Discussing the draft with" })).toBeNull(),
    );
  });

  it("closes the docked chat of the bot removed", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("itemInvite", () => ({
      case: "item",
      value: nobodyInvited(),
    }));
    render(<Drawer client={client} discuss="lead" />);
    expect(
      await screen.findByRole("complementary", { name: "Team Lead about RB-20" }),
    ).toBeInTheDocument();
    await user.click(
      await screen.findByRole("button", { name: "Remove Team Lead from the discussion" }),
    );
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Remove" }));
    await vi.waitFor(() =>
      expect(screen.queryByRole("complementary", { name: "Team Lead about RB-20" })).toBeNull(),
    );
  });

  it("follows a removal made on another device: the card's invitations read the draft again", async () => {
    const client = boardsClient();
    const { rerender } = render(<Drawer client={client} invited={["lead"]} discuss="lead" />);
    expect(
      await screen.findByRole("complementary", { name: "Team Lead about RB-20" }),
    ).toBeInTheDocument();
    client.onBoard("itemGet", () => ({ case: "item", value: nobodyInvited() }));
    rerender(<Drawer client={client} invited={[]} discuss="lead" />);
    await vi.waitFor(() =>
      expect(screen.queryByRole("complementary", { name: "Team Lead about RB-20" })).toBeNull(),
    );
    expect(screen.queryByRole("region", { name: "Discussing the draft with" })).toBeNull();
  });

  it("says why a removal failed", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("itemInvite", () => {
      throw new Error("RB-20 isn't a draft; only a draft takes invitations");
    });
    render(<Drawer client={client} />);
    await user.click(
      await screen.findByRole("button", { name: "Remove Team Lead from the discussion" }),
    );
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Remove" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("only a draft takes invitations");
  });
});

import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ItemDetail } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { draftDetail, recipeColumns } from "../../../test/boardsFixtures";
import { boardsClient, STORY_BOTS } from "../../../test/boardStory";
import ItemDrawer from "../drawer/ItemDrawer";

/** A reply held back until the test lets it go. */
function gate(): { readonly wait: Promise<void>; readonly open: () => void } {
  const held: (() => void)[] = [];
  const wait = new Promise<void>((resolve) => {
    held.push(resolve);
  });
  return {
    wait,
    open: () => {
      for (const release of held) {
        release();
      }
    },
  };
}

function invitedTo(invited: readonly string[]): ItemDetail {
  const detail = draftDetail();
  detail.invited = [...invited];
  return detail;
}

function renderDrawer(client: ReturnType<typeof boardsClient>): void {
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
    />,
  );
}

const invites = (client: ReturnType<typeof boardsClient>) =>
  client.boardCalls.filter((c) => c.case === "itemInvite");

describe("an invite on its way", () => {
  it("disables Discuss with a bot until the reply, so a second click sends nothing", async () => {
    const user = userEvent.setup();
    const reply = gate();
    const client = boardsClient().onBoard("itemInvite", async () => {
      await reply.wait;
      return { case: "item", value: invitedTo(["lead", "b2"]) };
    });
    renderDrawer(client);
    const discuss = await screen.findByRole("button", { name: "Discuss with a bot" });
    await user.click(discuss);
    await user.click(screen.getByRole("menuitem", { name: /Tester/ }));
    expect(discuss).toBeDisabled();
    await user.dblClick(discuss);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(invites(client)).toHaveLength(1);

    reply.open();
    await vi.waitFor(() => expect(discuss).toBeEnabled());
    expect(
      await screen.findByRole("complementary", { name: "Tester about RB-20" }),
    ).toBeInTheDocument();
    expect(invites(client)).toHaveLength(1);
  });

  it("enables it again after a refusal, and says why", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("itemInvite", () => {
      throw new Error("bot b2 is archived");
    });
    renderDrawer(client);
    const discuss = await screen.findByRole("button", { name: "Discuss with a bot" });
    await user.click(discuss);
    await user.click(screen.getByRole("menuitem", { name: /Tester/ }));
    expect(await screen.findByRole("alert")).toHaveTextContent("bot b2 is archived");
    expect(discuss).toBeEnabled();
    await user.click(discuss);
    expect(screen.getByRole("menu", { name: "Discuss the draft with" })).toBeInTheDocument();
  });
});

describe("a removal on its way", () => {
  it("asks once and sends once, even on a double click", async () => {
    const user = userEvent.setup();
    const reply = gate();
    const client = boardsClient().onBoard("itemInvite", async () => {
      await reply.wait;
      return { case: "item", value: invitedTo([]) };
    });
    renderDrawer(client);
    const remove = await screen.findByRole("button", {
      name: "Remove Team Lead from the discussion",
    });
    await user.click(remove);
    const dialog = screen.getByRole("dialog", { name: "Remove Team Lead from RB-20?" });
    await user.dblClick(within(dialog).getByRole("button", { name: "Remove" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    // Until the reply, the bot's Remove waits: no second confirm.
    expect(remove).toBeDisabled();
    await user.click(remove);
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(invites(client)).toHaveLength(1);

    reply.open();
    await vi.waitFor(() =>
      expect(screen.queryByRole("region", { name: "Discussing the draft with" })).toBeNull(),
    );
    expect(invites(client)).toHaveLength(1);
  });

  it("enables Remove again after a refusal", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("itemInvite", () => {
      throw new Error("RB-20 isn't a draft; only a draft takes invitations");
    });
    renderDrawer(client);
    const remove = await screen.findByRole("button", {
      name: "Remove Team Lead from the discussion",
    });
    await user.click(remove);
    await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Remove" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("only a draft takes invitations");
    expect(remove).toBeEnabled();
    await user.click(remove);
    expect(
      screen.getByRole("dialog", { name: "Remove Team Lead from RB-20?" }),
    ).toBeInTheDocument();
  });
});

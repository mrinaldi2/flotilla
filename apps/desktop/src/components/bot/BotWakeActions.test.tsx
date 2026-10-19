import { render, renderHook, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Bot, NotifyLevel } from "../../protocol/entities";
import { FakeDaemon } from "../../test/fakeDaemon";
import * as fx from "../../test/fixtures";
import { teamDaemon } from "../../test/teamFixtures";
import type { BotTab } from "./BotTabs";
import BotWakeActions from "./BotWakeActions";
import { useWakeOnOpen } from "./useWakeOnOpen";

type Toast = (level: NotifyLevel, title: string, body: string) => void;

function renderActions(bot: Bot, client = teamDaemon(), canControl = true) {
  const onToast = vi.fn<Toast>();
  const view = render(
    <BotWakeActions
      client={client}
      bot={bot}
      connected
      canControl={canControl}
      onToast={onToast}
    />,
  );
  return { client, onToast, ...view };
}

describe("Wake now and Sleep now in the bot's header", () => {
  it("wakes an asleep bot", async () => {
    const { client, onToast } = renderActions(fx.bot({ state: "asleep", presence: "on_demand" }));
    await userEvent.click(screen.getByRole("button", { name: "Wake now" }));
    expect(client.requests.at(-1)?.body).toEqual({ type: "wake_bot", bot_id: "b1" });
    expect(onToast).toHaveBeenCalledWith("info", "Waking alice", expect.any(String));
  });

  it("wakes a bot waiting for a slot ahead of board work", () => {
    renderActions(fx.bot({ state: "queued", presence: "on_demand" }));
    expect(screen.getByRole("button", { name: "Wake now" })).toBeEnabled();
  });

  it("sends an on-demand bot that is up to sleep", async () => {
    const { client } = renderActions(fx.bot({ state: "working", presence: "on_demand" }));
    await userEvent.click(screen.getByRole("button", { name: "Sleep now" }));
    expect(client.requests.at(-1)?.body).toEqual({ type: "sleep_bot", bot_id: "b1" });
  });

  it("offers nothing for an always-on bot, read-only, or before 0.23", () => {
    const always = renderActions(fx.bot({ presence: "always_on" }));
    expect(always.container).toBeEmptyDOMElement();
    const readOnly = renderActions(fx.bot({ state: "asleep" }), teamDaemon(), false);
    expect(readOnly.container).toBeEmptyDOMElement();
    const old = renderActions(fx.bot({ state: "asleep" }), new FakeDaemon());
    expect(old.container).toBeEmptyDOMElement();
  });
});

function openOn(bot: Bot, tab: BotTab, client = teamDaemon()) {
  const hook = renderHook(
    (props: { bot: Bot; tab: BotTab }) => {
      useWakeOnOpen(client, props.bot, props.tab, true, true);
    },
    { initialProps: { bot, tab } },
  );
  return { client, ...hook };
}

describe("opening an asleep bot's chat", () => {
  it("wakes it once", () => {
    const asleep = fx.bot({ state: "asleep" });
    const { client, rerender } = openOn(asleep, "chat");
    rerender({ bot: { ...asleep }, tab: "terminal" });
    expect(client.requests.map((r) => r.body)).toEqual([{ type: "wake_bot", bot_id: "b1" }]);
  });

  it("wakes it again after it slept again", () => {
    const { client, rerender } = openOn(fx.bot({ state: "asleep" }), "chat");
    rerender({ bot: fx.bot({ state: "ready" }), tab: "chat" });
    rerender({ bot: fx.bot({ state: "asleep" }), tab: "chat" });
    expect(client.requests).toHaveLength(2);
  });

  it("leaves it asleep on Reports, awake bots alone, and older services", () => {
    expect(openOn(fx.bot({ state: "asleep" }), "reports").client.requests).toEqual([]);
    expect(openOn(fx.bot({ state: "ready" }), "chat").client.requests).toEqual([]);
    const old = new FakeDaemon();
    openOn(fx.bot({ state: "asleep" }), "chat", old);
    expect(old.requests).toEqual([]);
  });
});

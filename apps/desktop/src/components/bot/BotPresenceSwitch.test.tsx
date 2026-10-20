import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Bot, NotifyLevel } from "../../protocol/entities";
import { FakeDaemon } from "../../test/fakeDaemon";
import * as fx from "../../test/fixtures";
import { teamDaemon } from "../../test/teamFixtures";
import BotPresenceSwitch from "./BotPresenceSwitch";

type Toast = (level: NotifyLevel, title: string, body: string) => void;

function renderSwitch(bot: Bot, lead = false, client = teamDaemon()) {
  const onBotUpdated = vi.fn<(bot: Bot) => void>();
  const onToast = vi.fn<Toast>();
  const view = render(
    <BotPresenceSwitch
      client={client}
      bot={bot}
      lead={lead}
      connected
      canControl
      onBotUpdated={onBotUpdated}
      onToast={onToast}
    />,
  );
  return { client, onBotUpdated, onToast, ...view };
}

describe("Always on / On demand", () => {
  it("keeps an on-demand bot running", async () => {
    const client = teamDaemon().onRequest("set_bot_presence", () => ({
      type: "bot",
      req_id: "1",
      bot: fx.bot({ presence: "always_on" }),
    }));
    const { onBotUpdated, onToast } = renderSwitch(
      fx.bot({ presence: "on_demand" }),
      false,
      client,
    );
    const toggle = screen.getByRole("checkbox", { name: /^Always on/ });
    expect(toggle).not.toBeChecked();
    expect(screen.getByText(/Sleeps after a few idle minutes/)).toBeInTheDocument();
    await userEvent.click(toggle);
    expect(client.requests.at(-1)?.body).toEqual({
      type: "set_bot_presence",
      bot_id: "b1",
      presence: "always_on",
    });
    expect(onBotUpdated).toHaveBeenCalledWith(expect.objectContaining({ presence: "always_on" }));
    expect(onToast).toHaveBeenCalledWith("info", "Always on", expect.any(String));
  });

  it("lets an always-on bot sleep when idle, and reports a refusal", async () => {
    const client = teamDaemon().onRequest("set_bot_presence", () => {
      throw new Error("conflict");
    });
    const { onToast } = renderSwitch(fx.bot({ presence: "always_on" }), false, client);
    await userEvent.click(screen.getByRole("checkbox", { name: /^Always on/ }));
    expect(client.requests.at(-1)?.body).toMatchObject({ presence: "on_demand" });
    expect(onToast).toHaveBeenCalledWith("error", "Couldn't change when the bot runs", "conflict");
  });

  it("is on and can't be switched for the project's lead, or a linked bot", () => {
    renderSwitch(fx.bot({ presence: "on_demand" }), true);
    const toggle = screen.getByRole("checkbox", { name: /^Always on/ });
    expect(toggle).toBeChecked();
    expect(toggle).toBeDisabled();
    expect(
      screen.getByText("alice leads the project, and a lead is always on."),
    ).toBeInTheDocument();
  });

  it("can't be switched here for a bot on another computer", () => {
    renderSwitch(fx.bot({ presence: "on_demand", peer: { id: "p", name: "win", online: true } }));
    expect(screen.getByRole("checkbox", { name: /^Always on/ })).toBeDisabled();
    expect(screen.getByText(/runs on another computer/)).toBeInTheDocument();
  });

  it("is absent before 0.23 and for a temporary worker", () => {
    expect(renderSwitch(fx.bot(), false, new FakeDaemon()).container).toBeEmptyDOMElement();
    expect(renderSwitch(fx.bot({ temporary: true })).container).toBeEmptyDOMElement();
  });
});

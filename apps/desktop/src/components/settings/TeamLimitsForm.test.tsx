import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { DaemonConfig } from "../../protocol/entities";
import { toastSpy } from "../../test/spies";
import { teamDaemon } from "../../test/teamFixtures";
import TeamLimitsForm from "./TeamLimitsForm";

const CONFIG: DaemonConfig = {
  bind: ["127.0.0.1"],
  port: 49777,
  configured_port: 49777,
  runtime: "pty",
  auto_compact_window: null,
  max_running_bots: 6,
  idle_sleep_minutes: 10,
};

function renderForm(config: DaemonConfig = CONFIG) {
  const client = teamDaemon().onRequest("set_config", (body) => ({
    type: "config",
    req_id: "1",
    config: { ...config, ...(body.type === "set_config" ? body : {}) } as DaemonConfig,
  }));
  const onSaved = vi.fn<(config: DaemonConfig) => void>();
  const onToast = toastSpy();
  render(
    <TeamLimitsForm
      client={client}
      config={config}
      connected
      canControl
      onSaved={onSaved}
      onToast={onToast}
    />,
  );
  return { client, onSaved, onToast };
}

describe("the running cap and idle time in Settings", () => {
  it("saves both with set_config", async () => {
    const user = userEvent.setup();
    const { client, onSaved, onToast } = renderForm();
    const save = screen.getByRole("button", { name: "Save" });
    expect(save).toBeDisabled();
    await user.clear(screen.getByRole("textbox", { name: "Bots running at once" }));
    await user.type(screen.getByRole("textbox", { name: "Bots running at once" }), "4");
    await user.clear(screen.getByRole("textbox", { name: "Sleep after" }));
    await user.type(screen.getByRole("textbox", { name: "Sleep after" }), "30");
    await user.click(save);
    expect(client.requests.at(-1)?.body).toEqual({
      type: "set_config",
      max_running_bots: 4,
      idle_sleep_minutes: 30,
    });
    expect(onSaved).toHaveBeenCalledWith(
      expect.objectContaining({ max_running_bots: 4, idle_sleep_minutes: 30 }),
    );
    expect(onToast).toHaveBeenCalledWith("info", "Settings saved", expect.any(String));
  });

  it("refuses values out of range before sending", async () => {
    const user = userEvent.setup();
    const { client } = renderForm();
    const running = screen.getByRole("textbox", { name: "Bots running at once" });
    await user.clear(running);
    await user.type(running, "33");
    expect(screen.getByText("Enter 1–32 bots and 1–1440 minutes.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    await user.clear(running);
    await user.type(running, "0");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    expect(client.requests).toEqual([]);
  });

  it("shows the defaults when the service leaves them out", () => {
    renderForm({ ...CONFIG, max_running_bots: undefined, idle_sleep_minutes: undefined });
    expect(screen.getByRole("textbox", { name: "Bots running at once" })).toHaveValue("6");
    expect(screen.getByRole("textbox", { name: "Sleep after" })).toHaveValue("10");
  });

  it("reports a refusal", async () => {
    const user = userEvent.setup();
    const { client, onToast } = renderForm();
    client.onRequest("set_config", () => {
      throw new Error("max_running_bots must be 1–32");
    });
    await user.clear(screen.getByRole("textbox", { name: "Sleep after" }));
    await user.type(screen.getByRole("textbox", { name: "Sleep after" }), "5");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onToast).toHaveBeenCalledWith(
      "error",
      "Couldn't save the setting",
      "max_running_bots must be 1–32",
    );
  });
});

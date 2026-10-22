import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { DaemonError } from "../../protocol/connection";
import type { Project } from "../../protocol/entities";
import { FakeDaemon } from "../../test/fakeDaemon";
import * as fx from "../../test/fixtures";
import { toastSpy } from "../../test/spies";
import { PHD_BUDGET, usageDaemon } from "../../test/usageFixtures";
import UsageBudget from "./UsageBudgetForm";

function renderForm(
  project: Project,
  opts: { readonly owner?: boolean; readonly focus?: boolean } = {},
) {
  const client = usageDaemon({ owner: opts.owner ?? true }).onRequest(
    "set_project_budget",
    (body) => {
      const weekly = body.type === "set_project_budget" ? body.weekly_tokens : null;
      return {
        type: "project",
        req_id: "1",
        project: {
          ...project,
          budget: weekly === null ? null : { ...PHD_BUDGET, weekly_tokens: weekly },
        },
      };
    },
  );
  const onToast = toastSpy();
  render(
    <UsageBudget
      client={client}
      project={project}
      connected
      onToast={onToast}
      focus={opts.focus}
    />,
  );
  return { client, onToast };
}

const field = (): HTMLElement =>
  screen.getByRole("textbox", { name: /^Weekly budget, in million tokens/ });

describe("a project's usage budget", () => {
  it("shows the budget in millions, with what the last 7 days used", () => {
    renderForm(fx.project({ budget: PHD_BUDGET }));
    expect(field()).toHaveValue("40");
    expect(screen.getByText(/29.8 M of 40 M used · 75% · last 7 days/)).toBeVisible();
    expect(
      screen.getByText(/on-demand bots stop waking; the lead and always-on bots keep going/),
    ).toBeVisible();
  });

  it("sends whole tokens for a budget in millions with decimals", async () => {
    const user = userEvent.setup();
    const { client, onToast } = renderForm(fx.project());
    expect(screen.queryByRole("button", { name: "No budget" })).not.toBeInTheDocument();
    await user.type(field(), "2.5000004");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(client.requests.at(-1)?.body).toEqual({
      type: "set_project_budget",
      project_id: "p1",
      weekly_tokens: 2_500_000,
    });
    expect(onToast).toHaveBeenCalledWith(
      "info",
      "Usage budget set",
      "Acme's bots may use 2.5 M tokens a week.",
    );
  });

  it("removes the budget with No budget", async () => {
    const user = userEvent.setup();
    const { client, onToast } = renderForm(fx.project({ budget: PHD_BUDGET }));
    await user.click(screen.getByRole("button", { name: "No budget" }));
    expect(client.requests.at(-1)?.body).toEqual({
      type: "set_project_budget",
      project_id: "p1",
      weekly_tokens: null,
    });
    expect(onToast).toHaveBeenCalledWith("info", "No usage budget", expect.any(String));
  });

  it("refuses what isn't a budget, and reports a refusal", async () => {
    const user = userEvent.setup();
    const { client, onToast } = renderForm(fx.project());
    await user.type(field(), "0");
    expect(screen.getByRole("alert")).toHaveTextContent("Enter millions of tokens above 0");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    await user.clear(field());
    await user.type(field(), "12");
    client.onRequest("set_project_budget", () => {
      throw new DaemonError("forbidden", "only the owner's app or a paired device sets this");
    });
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(onToast).toHaveBeenCalledWith(
      "error",
      "Couldn't set the usage budget",
      expect.stringContaining("only the owner"),
    );
  });

  it("is the owner's alone", () => {
    renderForm(fx.project({ budget: PHD_BUDGET }), { owner: false });
    expect(field()).toBeDisabled();
    expect(screen.getByText(/Only the owner sets this/)).toBeVisible();
    expect(screen.queryByRole("button", { name: "Save" })).not.toBeInTheDocument();
  });

  it("says when the budget is used up", () => {
    renderForm(fx.project({ budget: { ...PHD_BUDGET, used: 41_000_000, reached: true } }));
    expect(screen.getByText(/reached: its on-demand bots are held/)).toBeVisible();
  });

  it("takes focus when opened on it", () => {
    renderForm(fx.project(), { focus: true });
    expect(screen.getByRole("heading", { name: "Usage budget" })).toHaveFocus();
  });

  it("isn't there on a service that doesn't count tokens", () => {
    render(
      <UsageBudget
        client={new FakeDaemon()}
        project={fx.project()}
        connected
        onToast={toastSpy()}
      />,
    );
    expect(screen.queryByRole("heading", { name: "Usage budget" })).not.toBeInTheDocument();
  });
});

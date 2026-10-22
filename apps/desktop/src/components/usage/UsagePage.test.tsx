import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { FakeDaemon } from "../../test/fakeDaemon";
import * as fx from "../../test/fixtures";
import { USAGE_NOW, usageDaemon, weekUsage } from "../../test/usageFixtures";
import UsagePage from "./UsagePage";

const PROJECTS = [
  fx.project({ id: "p1", name: "PhD" }),
  fx.project({ id: "p2", name: "Books" }),
  fx.project({ id: "p3", name: "Flotilla" }),
];

function renderPage(client = usageDaemon()) {
  const onOpenBudget = vi.fn<(projectId: string) => void>();
  render(
    <UsagePage
      client={client}
      projects={PROJECTS}
      bots={[fx.bot({ id: "b9", name: "Old Timer" })]}
      connected
      onOpenBudget={onOpenBudget}
      now={USAGE_NOW}
    />,
  );
  return { client, onOpenBudget };
}

describe("the Usage page", () => {
  it("shows the last 7 days by default: totals, today, cache reads and the estimate", async () => {
    renderPage();
    expect(screen.getByRole("radio", { name: "7 days" })).toHaveAttribute("aria-checked", "true");
    expect(await screen.findByText("41.2 M tokens")).toBeInTheDocument();
    expect(screen.getByText("All projects, the last 7 days")).toBeInTheDocument();
    expect(screen.getByText("≈ $61.87 estimated · 1,284 turns")).toBeInTheDocument();
    expect(screen.getByText("6.8 M")).toBeInTheDocument();
    expect(screen.getByText(/on yesterday/)).toBeInTheDocument();
    expect(screen.getByText("87%")).toBeInTheDocument();
    expect(screen.getByText(/^from your price table, as of/)).toBeInTheDocument();
  });

  it("puts each project against its weekly budget, and opens its budget", async () => {
    const user = userEvent.setup();
    const { onOpenBudget } = renderPage();
    const projects = await screen.findByRole("region", { name: /By project/ });
    expect(within(projects).getByText("29.8 M of 40 M · 75%")).toBeInTheDocument();
    expect(within(projects).getByText("On track")).toBeInTheDocument();
    expect(within(projects).getByText("9.6 M of 10 M · 96%")).toBeInTheDocument();
    expect(within(projects).getByText("Nearly there")).toBeInTheDocument();
    expect(within(projects).getByText("No weekly budget.")).toBeInTheDocument();
    expect(
      within(projects).getAllByText(
        "At 100% its on-demand bots stop waking; the lead keeps going.",
      ),
    ).toHaveLength(2);
    await user.click(within(projects).getByRole("button", { name: "Change Books's budget" }));
    expect(onOpenBudget).toHaveBeenCalledWith("p2");
    await user.click(within(projects).getByRole("button", { name: "Set Flotilla's budget" }));
    expect(onOpenBudget).toHaveBeenCalledWith("p3");
    expect(
      within(projects).getByRole("img", { name: /^Tokens per day: Mon 5.1 M, Tue 7.4 M/ }),
    ).toHaveAccessibleName(expect.stringMatching(/Today 6.8 M$/));
  });

  it("lists the bots, for every project or one", async () => {
    const user = userEvent.setup();
    renderPage();
    const bots = await screen.findByRole("region", { name: "By bot" });
    expect(within(bots).getAllByRole("row")).toHaveLength(8);
    expect(within(bots).getByRole("row", { name: /Phd Game Lead 9.1 M 312 22%/ })).toBeVisible();
    await user.selectOptions(within(bots).getByRole("combobox", { name: "Project" }), "Books");
    expect(within(bots).getAllByRole("row")).toHaveLength(3);
    expect(within(bots).getByRole("row", { name: /Cover Artist 5.1 M 160 53%/ })).toBeVisible();
  });

  it("reads today when asked, without a day before or day bars", async () => {
    const user = userEvent.setup();
    const { client } = renderPage();
    await screen.findByText("41.2 M tokens");
    await user.click(screen.getByRole("radio", { name: "Today" }));
    expect(await screen.findByText("All projects, today")).toBeInTheDocument();
    expect(screen.getByText("tokens so far")).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: /Tokens per day/ })).not.toBeInTheDocument();
    expect(client.requests.at(-1)?.body).toEqual({ type: "usage_get", range: "today" });
  });

  it("names a bot the service no longer names from this computer, else as removed", async () => {
    const week = weekUsage();
    const [first, second] = week.bots;
    if (first === undefined || second === undefined) {
      throw new Error("the fixture has bots");
    }
    const client = usageDaemon().onRequest("usage_get", () => ({
      type: "usage",
      req_id: "1",
      ...week,
      bots: [
        { ...first, bot_id: "b9", name: "" },
        { ...second, bot_id: "gone", name: "" },
      ],
    }));
    renderPage(client);
    expect(await screen.findByRole("rowheader", { name: "Old Timer" })).toBeVisible();
    expect(screen.getByRole("rowheader", { name: "A removed bot" })).toBeVisible();
  });

  it("says when nothing was used", async () => {
    const client = usageDaemon().onRequest("usage_get", () => ({
      type: "usage",
      req_id: "1",
      range: "7d",
      total: {
        input: 0,
        output: 0,
        cache_read: 0,
        cache_write: 0,
        tokens: 0,
        turns: 0,
        est_cost_usd: 0,
      },
      projects: [],
      bots: [],
      days: [],
      prices_as_of: "",
    }));
    renderPage(client);
    expect(await screen.findByText("No project used any tokens in this period.")).toBeVisible();
    expect(screen.getByText("No bot used any tokens in this period.")).toBeVisible();
    expect(screen.getByText("—")).toBeVisible();
    expect(screen.getByText("from Flotilla's estimated prices")).toBeVisible();
  });

  it("asks for an update on a service that doesn't count tokens", () => {
    renderPage(new FakeDaemon());
    expect(screen.getByText(/doesn't count tokens yet/)).toBeInTheDocument();
  });

  it("says why it couldn't read usage", async () => {
    const client = usageDaemon().onRequest("usage_get", () => {
      throw new Error("no table");
    });
    renderPage(client);
    expect(await screen.findByText("Couldn't read usage: no table")).toBeVisible();
  });
});

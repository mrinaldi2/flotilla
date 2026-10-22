import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DaemonError } from "../../protocol/connection";
import { usageDaemon } from "../../test/usageFixtures";
import PriceTable from "./PriceTable";

function nth(list: readonly HTMLElement[], index: number): HTMLElement {
  const item = list.at(index);
  if (item === undefined) {
    throw new Error(`no item ${index}`);
  }
  return item;
}

function renderTable(client = usageDaemon({ owner: true })) {
  const onSaved = vi.fn<() => void>();
  render(<PriceTable client={client} connected onSaved={onSaved} />);
  return { client, onSaved };
}

describe("the price table", () => {
  it("shows the seeded estimates as such", async () => {
    renderTable();
    expect(await screen.findByText(/Flotilla's own estimates/)).toBeVisible();
    expect(screen.getByRole("textbox", { name: "opus input" })).toHaveValue("15");
    expect(screen.getByRole("button", { name: "Save prices" })).toBeDisabled();
  });

  it("saves the edited table, with a new model and without a removed one", async () => {
    const user = userEvent.setup();
    const { client, onSaved } = renderTable();
    const output = await screen.findByRole("textbox", { name: "opus output" });
    await user.clear(output);
    await user.type(output, "70.5");
    await user.click(screen.getByRole("button", { name: "Remove sonnet" }));
    await user.click(screen.getByRole("button", { name: "Add a model" }));
    const names = screen.getAllByRole("textbox", { name: "Model" });
    await user.type(nth(names, -1), "haiku");
    await user.type(screen.getByRole("textbox", { name: "haiku input" }), "1");
    await user.type(screen.getByRole("textbox", { name: "haiku output" }), "1");
    await user.type(screen.getByRole("textbox", { name: "haiku cache read" }), "1");
    await user.type(screen.getByRole("textbox", { name: "haiku cache write" }), "1");
    await user.click(screen.getByRole("button", { name: "Save prices" }));
    expect(client.requests.at(-1)?.body).toEqual({
      type: "set_usage_prices",
      prices: [
        { model: "opus", input: 15, output: 70.5, cache_read: 1.5, cache_write: 18.75 },
        { model: "*", input: 3, output: 15, cache_read: 0.3, cache_write: 3.75 },
        { model: "haiku", input: 1, output: 1, cache_read: 1, cache_write: 1 },
      ],
    });
    expect(onSaved).toHaveBeenCalled();
    expect(await screen.findByText(/^Your prices, set/)).toBeVisible();
  });

  it("refuses what isn't a price, a name twice, and keeps the * row", async () => {
    const user = userEvent.setup();
    renderTable();
    const input = await screen.findByRole("textbox", { name: "opus input" });
    await user.clear(input);
    await user.type(input, "-2");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "opus: every price is a number of dollars, 0 or more.",
    );
    expect(screen.getByRole("button", { name: "Save prices" })).toBeDisabled();
    await user.clear(input);
    await user.type(input, "15");
    const names = screen.getAllByRole("textbox", { name: "Model" });
    await user.clear(nth(names, 1));
    await user.type(nth(names, 1), "Opus");
    expect(screen.getByRole("alert")).toHaveTextContent("opus is listed twice.");
    await user.clear(nth(names, 1));
    expect(screen.getByRole("alert")).toHaveTextContent("Name every model");
    expect(screen.queryByRole("button", { name: "Remove *" })).not.toBeInTheDocument();
    expect(screen.getByDisplayValue("*")).toHaveAttribute("readonly");
  });

  it("says why the service refused the table", async () => {
    const user = userEvent.setup();
    const client = usageDaemon({ owner: true }).onRequest("set_usage_prices", () => {
      throw new DaemonError("forbidden", "only the owner's app or a paired device sets this");
    });
    const { onSaved } = renderTable(client);
    const input = await screen.findByRole("textbox", { name: "opus input" });
    await user.type(input, "0");
    await user.click(screen.getByRole("button", { name: "Save prices" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Couldn't save the prices: only the owner's app",
    );
    expect(onSaved).not.toHaveBeenCalled();
  });

  it("is read only without the owner's grant", async () => {
    renderTable(usageDaemon());
    const table = await screen.findByRole("table");
    expect(within(table).getByRole("row", { name: "opus $15 $75 $1.5 $18.75" })).toBeVisible();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Save prices" })).not.toBeInTheDocument();
    expect(screen.getByText(/Only the owner changes prices/)).toBeVisible();
  });

  it("says why it couldn't load the prices", async () => {
    const client = usageDaemon().onRequest("usage_prices", () => {
      throw new Error("offline");
    });
    renderTable(client);
    expect(await screen.findByText("Couldn't load: offline")).toBeVisible();
  });
});

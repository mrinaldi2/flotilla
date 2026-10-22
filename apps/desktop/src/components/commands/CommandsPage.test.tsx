import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import {
  COMMANDS_NOW,
  commandBots,
  commandProjects,
  commandRow,
  commandsDaemon,
  commandsReply,
} from "../../test/commandFixtures";
import type { FakeDaemon } from "../../test/fakeDaemon";
import CommandsPage from "./CommandsPage";

function show(client: FakeDaemon, connected = true) {
  const props = {
    onOpenBot: vi.fn<(botId: string) => void>(),
    onOpenItem: vi.fn<(projectId: string, itemId: string) => void>(),
  };
  render(
    <CommandsPage
      client={client}
      projects={commandProjects}
      bots={commandBots}
      connected={connected}
      now={COMMANDS_NOW}
      {...props}
    />,
  );
  return props;
}

function lastList(client: FakeDaemon): unknown {
  const lists = client.requests.filter((r) => r.body.type === "list_commands");
  return lists[lists.length - 1]?.body;
}

const rowButton = (name: RegExp): Promise<HTMLElement> =>
  within(screen.getByRole("region", { name: "Commands" })).findByRole("button", { name });

describe("CommandsPage", () => {
  it("lists every bot's commands with the summary on top", async () => {
    show(commandsDaemon());
    const train = await rowButton(/dungeonflow\.train/);
    expect(within(train).getByText("◑ Running")).toBeInTheDocument();
    expect(within(train).getByText("PhD · mac")).toBeInTheDocument();
    expect(within(train).getByText("Train the RL baseline · background")).toBeInTheDocument();
    expect(within(train).getByText("18m 00s")).toBeInTheDocument();
    expect(within(await rowButton(/render/)).getByText("Books · win-pc")).toBeInTheDocument();
    expect(
      within(await rowButton(/playwright/)).getByText("✗ Failed · exit 1"),
    ).toBeInTheDocument();
    expect(within(await rowButton(/eval/)).getByText("⊘ Stopped by you")).toBeInTheDocument();
    expect(screen.getByText("mac 2 · win-pc 1")).toBeInTheDocument();
    expect(screen.getByText("failed today").previousSibling).toHaveTextContent("1");
    expect(screen.getByText("Pick a command to see its output.")).toBeInTheDocument();
  });

  it("shows a selected command's detail and output", async () => {
    const props = show(commandsDaemon());
    await userEvent.click(await rowButton(/playwright/));
    const detail = screen.getByRole("complementary", { name: "Command detail" });
    expect(within(detail).getByText(/Timed out 30000ms/)).toBeInTheDocument();
    expect(within(detail).queryByRole("button", { name: "Stop" })).not.toBeInTheDocument();
    await userEvent.click(within(detail).getByRole("button", { name: "CASS-049" }));
    expect(props.onOpenItem).toHaveBeenCalledWith("p1", "CASS-049");
    await userEvent.click(within(detail).getByRole("button", { name: "Open bot" }));
    expect(props.onOpenBot).toHaveBeenCalledWith("b3");
  });

  it("offers Stop only to the owner, on a running background command", async () => {
    const client = commandsDaemon();
    show(client);
    await userEvent.click(await rowButton(/Unity -batchmode/));
    expect(screen.queryByRole("button", { name: "Stop" })).not.toBeInTheDocument();
    await userEvent.click(await rowButton(/dungeonflow\.train/));
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    await userEvent.click(screen.getByRole("button", { name: "Stop command" }));
    await waitFor(() => {
      expect(client.requests.map((r) => r.body)).toContainEqual({
        type: "stop_command",
        bot_id: "b1",
        command_id: "c1",
      });
    });
    const stop = client.requests.findIndex((r) => r.body.type === "stop_command");
    await waitFor(() => {
      expect(client.requests.slice(stop + 1).some((r) => r.body.type === "list_commands")).toBe(
        true,
      );
    });
  });

  it("hides Stop without the owner's grant", async () => {
    const client = commandsDaemon();
    client.grants = ["read", "control"];
    show(client);
    await userEvent.click(await rowButton(/dungeonflow\.train/));
    expect(screen.queryByRole("button", { name: "Stop" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open bot" })).toBeInTheDocument();
  });

  it("filters by project, bot, status and search", async () => {
    const client = commandsDaemon();
    show(client);
    await rowButton(/dungeonflow\.train/);
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Bot" }), "b4");
    await userEvent.selectOptions(screen.getByRole("combobox", { name: "Project" }), "p1");
    // Book Cover is Books': choosing PhD lets go of it, and lists PhD's bots only.
    expect(screen.getByRole("combobox", { name: "Bot" })).toHaveValue("");
    expect(screen.queryByRole("option", { name: "Book Cover" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: "Failed" }));
    expect(screen.getByRole("radio", { name: "Failed" })).toHaveAttribute("aria-checked", "true");
    await userEvent.type(screen.getByRole("searchbox", { name: "Search commands" }), "tour");
    await waitFor(() => {
      expect(lastList(client)).toEqual({
        type: "list_commands",
        project_id: "p1",
        status: "failed",
        query: "tour",
        limit: 50,
      });
    });
  });

  it("says when nothing matches, and when there is nothing yet", async () => {
    const client = commandsDaemon(() => commandsReply({ commands: [] }));
    show(client);
    expect(await screen.findByText("Your bots have not run any commands yet.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("radio", { name: "Running" }));
    expect(await screen.findByText("No commands match these filters.")).toBeInTheDocument();
  });

  it("shows a failure with Try again", async () => {
    let fail = true;
    const client = commandsDaemon(() => {
      if (fail) {
        throw new Error("peer is offline");
      }
      return commandsReply();
    });
    show(client);
    expect(await screen.findByText("peer is offline")).toBeInTheDocument();
    fail = false;
    await userEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(await rowButton(/dungeonflow\.train/)).toBeInTheDocument();
    expect(screen.queryByText("peer is offline")).not.toBeInTheDocument();
  });

  it("loads more when there is more", async () => {
    const client = commandsDaemon((body) =>
      body.type === "list_commands" && body.before !== undefined
        ? commandsReply({ commands: [commandRow({ id: "c9", command: "make older" })] })
        : commandsReply({ next_before: "c6" }),
    );
    show(client);
    await userEvent.click(await screen.findByRole("button", { name: "Load more" }));
    expect(await rowButton(/make older/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Load more" })).not.toBeInTheDocument();
  });

  it("says when it isn't connected", () => {
    const client = commandsDaemon();
    show(client, false);
    expect(screen.getByText("Not connected.")).toBeInTheDocument();
    expect(client.requests).toHaveLength(0);
  });
});

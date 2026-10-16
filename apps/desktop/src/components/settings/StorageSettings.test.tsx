import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { NotifyLevel } from "../../protocol/entities";
import { FakeDaemon } from "../../test/fakeDaemon";
import { DELETED_PREVIEW } from "../../test/projectTrashFixtures";
import StorageSettings from "./StorageSettings";

function renderPane(client: FakeDaemon) {
  const onToast = vi.fn<(level: NotifyLevel, title: string, body: string) => void>();
  render(<StorageSettings client={client} connected canControl onToast={onToast} />);
  return { onToast };
}

const LOOK = { name: "Clean up deleted projects" };

describe("StorageSettings", () => {
  it("shows what would move, and moves it only on confirmation", async () => {
    const user = userEvent.setup();
    const client = new FakeDaemon()
      .onRequest("deleted_projects_preview", () => ({
        type: "deleted_projects_preview",
        req_id: "1",
        preview: DELETED_PREVIEW,
      }))
      .onRequest("deleted_projects_cleanup", () => ({
        type: "deleted_projects_cleaned",
        req_id: "2",
        moved_bytes: 1_400_000_000,
        summary: "Moved 1.4 GB to the Trash: project files 1.4 GB.",
        skipped: [],
      }));
    const { onToast } = renderPane(client);

    await user.click(screen.getByRole("button", LOOK));
    const dialog = await screen.findByRole("dialog", { name: "Move 1.4 GB to the Trash?" });
    expect(
      within(dialog).getByText("This moves 1.4 GB to the Trash: Old 1.2 GB, Other 200 MB."),
    ).toBeInTheDocument();
    expect(screen.getByText("Old on win: didn't confirm")).toBeInTheDocument();
    expect(client.requests.map((r) => r.body.type)).toEqual(["deleted_projects_preview"]);

    await user.click(within(dialog).getByRole("button", { name: "Move to the trash" }));
    expect(client.requests.map((r) => r.body.type)).toEqual([
      "deleted_projects_preview",
      "deleted_projects_cleanup",
    ]);
    expect(onToast).toHaveBeenCalledWith(
      "info",
      "Deleted projects cleaned up",
      "Moved 1.4 GB to the Trash: project files 1.4 GB.",
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("says when there is nothing to clean up, and asks nothing", async () => {
    const user = userEvent.setup();
    const client = new FakeDaemon().onRequest("deleted_projects_preview", () => ({
      type: "deleted_projects_preview",
      req_id: "1",
      preview: { projects: [], total_bytes: 0, bin: "Trash", linked: [] },
    }));
    const { onToast } = renderPane(client);

    await user.click(screen.getByRole("button", LOOK));
    expect(onToast).toHaveBeenCalledWith(
      "info",
      "Nothing to clean up",
      "Deleted projects left nothing on this computer.",
    );
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("cancelling moves nothing", async () => {
    const user = userEvent.setup();
    const client = new FakeDaemon().onRequest("deleted_projects_preview", () => ({
      type: "deleted_projects_preview",
      req_id: "1",
      preview: DELETED_PREVIEW,
    }));
    renderPane(client);

    await user.click(screen.getByRole("button", LOOK));
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(client.requests.map((r) => r.body.type)).toEqual(["deleted_projects_preview"]);
  });

  it("is locked without control", () => {
    render(
      <StorageSettings
        client={new FakeDaemon()}
        connected
        canControl={false}
        onToast={vi.fn<(level: NotifyLevel, title: string, body: string) => void>()}
      />,
    );
    expect(screen.getByRole("button", LOOK)).toBeDisabled();
  });
});

import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { DaemonError } from "../../protocol/connection";
import { FakeDaemon } from "../../test/fakeDaemon";
import * as fx from "../../test/fixtures";
import { DELETE_PREVIEW } from "../../test/projectTrashFixtures";
import { voidSpy } from "../../test/spies";
import DeleteProjectDialog from "./DeleteProjectDialog";

function previewing(): FakeDaemon {
  return new FakeDaemon().onRequest("project_delete_preview", () => ({
    type: "project_delete_preview",
    req_id: "1",
    preview: DELETE_PREVIEW,
  }));
}

function renderDialog(client: FakeDaemon) {
  const onConfirm = voidSpy();
  const onCancel = voidSpy();
  render(
    <DeleteProjectDialog
      client={client}
      project={fx.project()}
      botCount={2}
      onConfirm={onConfirm}
      onCancel={onCancel}
    />,
  );
  return { onConfirm, onCancel, dialog: screen.getByRole("dialog", { name: "Delete Acme?" }) };
}

describe("DeleteProjectDialog", () => {
  it("says what moves to the trash, by kind, and which computers are told", async () => {
    const client = previewing();
    const { dialog } = renderDialog(client);

    expect(within(dialog).getByText(/2 bots are stopped and archived/)).toBeInTheDocument();
    expect(
      await within(dialog).findByText(
        "This moves 1.6 GB to the Trash: project files 380 MB, bot session history 1.2 GB, git cache 3 MB.",
      ),
    ).toBeInTheDocument();
    expect(
      within(dialog).getByText(
        "Linked computer told to trash its copy: win (offline, told when it's back).",
      ),
    ).toBeInTheDocument();
    expect(client.requests[0]?.body).toEqual({ type: "project_delete_preview", project_id: "p1" });
  });

  it("deletes only once the project's name is typed", async () => {
    const user = userEvent.setup();
    const { onConfirm, dialog } = renderDialog(previewing());
    const remove = within(dialog).getByRole("button", { name: "Delete project" });
    expect(remove).toBeDisabled();

    await user.type(within(dialog).getByLabelText("Type Acme to delete it"), "Acm");
    expect(remove).toBeDisabled();
    await user.type(within(dialog).getByLabelText("Type Acme to delete it"), "e");
    await user.click(remove);
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("still deletes when the service can't count, and says so", async () => {
    const user = userEvent.setup();
    const client = new FakeDaemon().onRequest("project_delete_preview", () => {
      throw new DaemonError("invalid_request", "unknown type: project_delete_preview");
    });
    const { onConfirm, dialog } = renderDialog(client);

    expect(await within(dialog).findByText(/couldn't be counted/)).toBeInTheDocument();
    await user.type(within(dialog).getByLabelText("Type Acme to delete it"), "Acme");
    await user.click(within(dialog).getByRole("button", { name: "Delete project" }));
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("cancels on Escape, with Cancel holding the focus", async () => {
    const user = userEvent.setup();
    const { onCancel, onConfirm, dialog } = renderDialog(previewing());
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalled();
    expect(onConfirm).not.toHaveBeenCalled();
  });
});

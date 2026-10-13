import { create } from "@bufbuild/protobuf";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { DaemonError } from "../../../protocol/connection";
import { EditResultSchema } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { unmet } from "../../../test/boardFixtures";
import { draftItem } from "../../../test/boardsFixtures";
import { STORY_BOTS } from "../../../test/boardStory";
import { FakeDaemon } from "../../../test/fakeDaemon";
import ItemEditForm from "./ItemEditForm";

function done() {
  return create(EditResultSchema, { outcome: { case: "done", value: draftItem({ version: 7n }) } });
}

function renderForm(fake: FakeDaemon, onPasteImage?: (file: File) => Promise<string | null>) {
  const onSaved = vi.fn<() => void>();
  render(
    <ItemEditForm
      api={fake}
      item={draftItem()}
      bots={STORY_BOTS}
      onPasteImage={onPasteImage}
      onSaved={onSaved}
      onCancel={vi.fn<() => void>()}
    />,
  );
  return onSaved;
}

describe("the owner's edit form", () => {
  it("saves what changed, then the assignee, with the version the save left", async () => {
    const user = userEvent.setup();
    const fake = new FakeDaemon()
      .onBoard("itemUpdate", () => ({ case: "edited", value: done() }))
      .onBoard("itemAssign", () => ({ case: "edited", value: done() }));
    const onSaved = renderForm(fake);
    await user.type(screen.getByRole("textbox", { name: "Labels" }), "kitchen");
    await user.selectOptions(screen.getByRole("combobox", { name: "Assignee" }), "b1");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(fake.boardCalls[0]).toMatchObject({
      case: "itemUpdate",
      value: { id: "RB-20", expectedVersion: 6n, labels: { values: ["kitchen"] } },
    });
    expect(fake.boardCalls[1]).toMatchObject({
      case: "itemAssign",
      value: { bot: "b1", expectedVersion: 7n },
    });
    expect(onSaved).toHaveBeenCalled();
  });

  it("shows a refusal, a conflict and a failure in words", async () => {
    const user = userEvent.setup();
    let reply = 0;
    const fake = new FakeDaemon().onBoard("itemUpdate", () => {
      reply += 1;
      if (reply === 1) {
        return {
          case: "edited",
          value: create(EditResultSchema, {
            outcome: { case: "refused", value: { unmet: [unmet("x", "Fixed once in Verify.")] } },
          }),
        };
      }
      if (reply === 2) {
        return {
          case: "edited",
          value: create(EditResultSchema, { outcome: { case: "conflict", value: draftItem() } }),
        };
      }
      throw new DaemonError("forbidden", "no control grant");
    });
    const onSaved = renderForm(fake);
    await user.type(screen.getByRole("textbox", { name: "Title" }), "!");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Fixed once in Verify.");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("changed while you edited it");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("no control grant");
    expect(onSaved).not.toHaveBeenCalled();
  });

  it("refuses a blank title before sending, and places a pasted screenshot inline", async () => {
    const user = userEvent.setup();
    const fake = new FakeDaemon();
    renderForm(fake, () => Promise.resolve("![shot](attachment:a1)"));
    await user.clear(screen.getByRole("textbox", { name: "Title" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Give it a title.");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    const description = screen.getByRole("textbox", { name: /^Description/ });
    await user.click(description);
    await user.paste({
      files: [new File(["png"], "shot.png", { type: "image/png" })],
      getData: () => "",
      types: ["Files"],
    } as unknown as DataTransfer);
    expect(await screen.findByDisplayValue(/attachment:a1/)).toBeInTheDocument();
  });
});

import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { UploadProgressSchema } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { draftDetail } from "../../../test/boardsFixtures";
import { boardsClient, STORY_BOTS } from "../../../test/boardStory";
import { recipeColumns } from "../../../test/boardsFixtures";
import ItemDrawer from "../drawer/ItemDrawer";

const noop = (): void => undefined;

function renderDrawer(client = boardsClient()) {
  render(
    <ItemDrawer
      api={client}
      itemId="RB-20"
      columns={recipeColumns()}
      bots={STORY_BOTS}
      canComment
      onClose={vi.fn<() => void>()}
    />,
  );
  return client;
}

describe("attachments in the drawer", () => {
  it("lists images, files and documents, and says which were detached", async () => {
    renderDrawer();
    const section = await screen.findByRole("region", { name: "Attachments" });
    expect(within(section).getByText("import-error.png · The error")).toBeInTheDocument();
    expect(within(section).getByText("pdf · 1.3 MB")).toBeInTheDocument();
    expect(within(section).getByText("document · specs/RB-20-spec.md")).toBeInTheDocument();
    expect(
      within(section).getByText(/Detached, kept in the history: first-try.png/),
    ).toBeInTheDocument();
  });

  it("refuses a file over its cap at once, without sending it", async () => {
    const user = userEvent.setup({ applyAccept: false });
    const client = renderDrawer();
    await screen.findByRole("region", { name: "Attachments" });
    const huge = new File([new Uint8Array(8)], "IMG_1.heic", { type: "image/heic" });
    await user.upload(screen.getByLabelText("Add files"), huge);
    expect(await screen.findByText(/Refused: HEIC isn't taken/)).toBeInTheDocument();
    expect(client.boardCalls.some((c) => c.case === "attachmentUpload")).toBe(false);
    await user.click(screen.getByRole("button", { name: "Dismiss IMG_1.heic" }));
    expect(screen.queryByText(/Refused: HEIC/)).toBeNull();
  });

  it("uploads a file, then attaches it", async () => {
    const user = userEvent.setup();
    const client = boardsClient()
      .onBoard("attachmentUpload", (call) => ({
        case: "upload",
        value: create(UploadProgressSchema, {
          received: call.case === "attachmentUpload" ? (call.value.total ?? 0n) : 0n,
        }),
      }))
      .onBoard("itemAttach", () => ({ case: "item", value: draftDetail() }));
    renderDrawer(client);
    await screen.findByRole("region", { name: "Attachments" });
    await user.upload(screen.getByLabelText("Add files"), new File(["log"], "run.log"));
    await vi.waitFor(() =>
      expect(client.boardCalls.some((c) => c.case === "itemAttach")).toBe(true),
    );
  });

  it("cancels an upload on its way: no more chunks, and nothing attached", async () => {
    const user = userEvent.setup();
    let release: () => void = noop;
    const client = boardsClient()
      .onBoard(
        "attachmentUpload",
        (call) =>
          new Promise((resolve) => {
            release = () =>
              resolve({
                case: "upload",
                value: create(UploadProgressSchema, {
                  received:
                    call.case === "attachmentUpload"
                      ? (call.value.offset ?? 0n) + BigInt(call.value.data?.length ?? 0)
                      : 0n,
                }),
              });
          }),
      )
      .onBoard("itemAttach", () => ({ case: "item", value: draftDetail() }));
    renderDrawer(client);
    await screen.findByRole("region", { name: "Attachments" });
    const big = new File([new Uint8Array(1_200_000)], "big.log");
    await user.upload(screen.getByLabelText("Add files"), big);
    await user.click(await screen.findByRole("button", { name: "Cancel uploading big.log" }));
    release();
    await vi.waitFor(() => expect(screen.queryByText("big.log")).toBeNull());
    const uploads = client.boardCalls.filter((c) => c.case === "attachmentUpload");
    expect(uploads).toHaveLength(1);
    expect(client.boardCalls.some((c) => c.case === "itemAttach")).toBe(false);
  });

  it("links a document by path or URL, and detaches one", async () => {
    const user = userEvent.setup();
    const client = boardsClient()
      .onBoard("itemAttach", () => ({ case: "item", value: draftDetail() }))
      .onBoard("itemDetach", () => ({ case: "item", value: draftDetail() }));
    renderDrawer(client);
    await user.click(await screen.findByRole("button", { name: "+ Link document" }));
    await user.type(
      screen.getByRole("textbox", { name: "Artifact path or URL" }),
      "https://x.dev/spec",
    );
    await user.click(screen.getByRole("button", { name: "Link" }));
    const link = client.boardCalls.find((c) => c.case === "itemAttach");
    expect(link?.case === "itemAttach" && link.value).toMatchObject({ url: "https://x.dev/spec" });
    await user.click(screen.getByRole("button", { name: "Detach chart.pdf" }));
    const detach = client.boardCalls.find((c) => c.case === "itemDetach");
    expect(detach?.case === "itemDetach" && detach.value).toMatchObject({
      attachmentId: "att-chart",
    });
  });

  it("opens an image full size", async () => {
    const user = userEvent.setup();
    renderDrawer();
    await user.click(await screen.findByRole("button", { name: "Open The error full size" }));
    expect(await screen.findByRole("dialog", { name: "import-error.png" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "import-error.png" })).toBeNull();
    // Focus goes back to the thumbnail that opened it.
    expect(screen.getByRole("button", { name: "Open The error full size" })).toHaveFocus();
  });
});

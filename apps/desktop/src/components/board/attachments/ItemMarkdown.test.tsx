import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ItemAttachment } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { draftDetail } from "../../../test/boardsFixtures";
import { boardsClient } from "../../../test/boardStory";
import ItemMarkdown from "./ItemMarkdown";

describe("an item's description", () => {
  it("names a file it shows inline that isn't an image, rather than reading it as one", async () => {
    const user = userEvent.setup();
    const client = boardsClient();
    const onOpen = vi.fn<(a: ItemAttachment) => void>();
    render(
      <ItemMarkdown
        api={client}
        text="See ![the chart](attachment:att-chart) and ![spec](attachment:att-spec)."
        attachments={draftDetail().attachments}
        onOpen={onOpen}
      />,
    );
    await user.click(screen.getByRole("button", { name: "Open chart.pdf" }));
    expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ id: "att-chart" }));
    expect(screen.getByRole("button", { name: "Open RB-20-spec.md" })).toBeInTheDocument();
    expect(screen.queryByRole("img")).toBeNull();
    expect(client.boardCalls.some((c) => c.case === "attachmentRead")).toBe(false);
  });
});

import { create } from "@bufbuild/protobuf";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ItemAttachmentSchema } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { AttachmentChunkSchema } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { boardsClient } from "../../../test/boardStory";
import AttachmentViewer from "./AttachmentViewer";

function attached(name: string, mime: string, text: string) {
  const data = new TextEncoder().encode(text);
  const client = boardsClient().onBoard("attachmentRead", () => ({
    case: "attachment",
    value: create(AttachmentChunkSchema, { offset: 0n, data, total: BigInt(data.length), mime }),
  }));
  const attachment = create(ItemAttachmentSchema, { id: "at-1", kind: "file", name, mime, bytes: BigInt(data.length) });
  return { client, attachment };
}

describe("a card's documents in the viewer (0.20.0)", () => {
  it("reads an attached Markdown file as formatted text", async () => {
    const { client, attachment } = attached("SPEC.md", "text/markdown", "# The spec\n\nIt **works**.");
    render(<AttachmentViewer api={client} attachment={attachment} onClose={vi.fn<() => void>()} />);
    expect(await screen.findByRole("heading", { name: "The spec" })).toBeInTheDocument();
  });

  it("shows an attached page in a sandbox with no network", async () => {
    const { client, attachment } = attached("report.html", "text/html", "<h1>Benchmarks</h1>");
    render(<AttachmentViewer api={client} attachment={attachment} onClose={vi.fn<() => void>()} />);
    const frame = await screen.findByTitle("report.html");
    expect(frame.getAttribute("sandbox")).toBe("allow-scripts");
    const page = frame.getAttribute("srcdoc") ?? "";
    expect(page).toContain("<h1>Benchmarks</h1>");
    expect(page).toContain("default-src 'none'");
  });

  it("reads a linked artifact page with its stylesheet beside it", async () => {
    const files: Record<string, { mime: string; text: string }> = {
      "reports/run/index.html": { mime: "text/html", text: '<link rel="stylesheet" href="s.css"><p>Run</p>' },
      "reports/run/s.css": { mime: "text/css", text: "p{color:teal}" },
    };
    const readArtifact = vi.fn(async (path: string) => {
      const file = files[path];
      if (file === undefined) {
        throw new Error(path);
      }
      return file;
    });
    const attachment = create(ItemAttachmentSchema, { id: "at-2", kind: "doc", name: "Run report", path: "reports/run/index.html" });
    render(
      <AttachmentViewer api={boardsClient()} attachment={attachment} readArtifact={readArtifact} onClose={vi.fn<() => void>()} />,
    );
    const frame = await screen.findByTitle("Run report");
    expect(frame.getAttribute("srcdoc")).toContain("p{color:teal}");
    expect(readArtifact).toHaveBeenCalledWith("reports/run/s.css");
  });

  it("says where to read a linked document it can't reach", async () => {
    const attachment = create(ItemAttachmentSchema, { id: "at-3", kind: "doc", name: "Spec", path: "specs/a.md" });
    render(<AttachmentViewer api={boardsClient()} attachment={attachment} onClose={vi.fn<() => void>()} />);
    expect(await screen.findByText(/computer that keeps its board/)).toBeInTheDocument();
  });
});

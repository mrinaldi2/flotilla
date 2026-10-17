// A card's document, read in the app (0.20.0): Markdown formatted, an HTML
// page in a sandbox with no network, text, PDFs and images.

import { useEffect, useMemo, useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../../protocol/api";
import type { BoardApi } from "../../../protocol/board";
import type { ItemAttachment } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { errText } from "../../../util";
import ChatMarkdown from "../../chat/ChatMarkdown";
import { inlinePage, kindOf, textOf, type Preview, type Reader } from "./preview";
import { dataUrl, readAttachment } from "./transfer";

/** Reads the project's artifacts through `read_file`; none without both. */
export function useArtifactReader(
  client: DaemonApi | undefined,
  projectId: string | undefined,
): Reader | null {
  return useMemo(() => {
    if (client === undefined || projectId === undefined) {
      return null;
    }
    return async (path: string) => {
      const reply = await client.request(
        { type: "read_file", project_id: projectId, path },
        "file",
      );
      return reply.file;
    };
  }, [client, projectId]);
}

/** An HTML page as the card's own: scripts run, nothing leaves it. */
export function SandboxedPage(props: {
  readonly page: string;
  readonly title: string;
}): ReactElement {
  return (
    <iframe
      className="document-page"
      title={props.title}
      sandbox="allow-scripts"
      referrerPolicy="no-referrer"
      srcDoc={props.page}
    />
  );
}

function PdfView(props: { readonly bytes: Uint8Array; readonly title: string }): ReactElement {
  const url = useMemo(
    () => URL.createObjectURL(new Blob([props.bytes as BlobPart], { type: "application/pdf" })),
    [props.bytes],
  );
  useEffect(() => () => URL.revokeObjectURL(url), [url]);
  return (
    <object className="document-page" type="application/pdf" data={url} aria-label={props.title}>
      <span className="lightbox-note">This PDF can&apos;t be shown here.</span>
    </object>
  );
}

function PreviewView(props: { readonly preview: Preview; readonly title: string }): ReactElement {
  const { preview } = props;
  switch (preview.kind) {
    case "markdown":
      return (
        <div className="document-text">
          <ChatMarkdown>{preview.text}</ChatMarkdown>
        </div>
      );
    case "html":
      return <SandboxedPage page={preview.page} title={props.title} />;
    case "text":
      return <pre className="document-text document-plain">{preview.text}</pre>;
    case "pdf":
      return <PdfView bytes={preview.bytes} title={props.title} />;
    case "image":
      return (
        <span className="lightbox-image">
          <img src={preview.url} alt={props.title} />
        </span>
      );
    case "none":
      return <span className="lightbox-note">{preview.why}</span>;
  }
}

/** A document's text or bytes as the preview of its kind. */
async function previewOf(
  kind: ReturnType<typeof kindOf>,
  body: { readonly text: string; readonly bytes: () => Uint8Array; readonly url: () => string },
  page: (html: string) => Promise<string>,
): Promise<Preview> {
  switch (kind) {
    case "html":
      return { kind: "html", page: await page(body.text) };
    case "markdown":
    case "text":
      return { kind, text: body.text };
    case "pdf":
      return { kind: "pdf", bytes: body.bytes() };
    case "image":
      return { kind: "image", url: body.url() };
    case null:
      return { kind: "none", why: "No preview for this kind of file." };
  }
}

function base64Bytes(base64: string | undefined): Uint8Array {
  return Uint8Array.from(atob(base64 ?? ""), (c) => c.charCodeAt(0));
}

/** A linked artifact, its page's files read beside it. */
async function artifactPreview(a: ItemAttachment, read: Reader | null): Promise<Preview> {
  if (a.path === "") {
    return { kind: "none", why: "A web address: open it in your browser." };
  }
  if (read === null) {
    return { kind: "none", why: "Open this card on the computer that keeps its board to read it." };
  }
  const file = await read(a.path);
  return previewOf(
    kindOf(file.mime, a.path),
    {
      text: file.text ?? "",
      bytes: () => base64Bytes(file.base64),
      url: () => `data:${file.mime};base64,${file.base64 ?? ""}`,
    },
    (html) => inlinePage(html, a.path, read),
  );
}

/** An attached file: one file, so a page's files beside it aren't there. */
async function filePreview(api: BoardApi, a: ItemAttachment): Promise<Preview> {
  const { bytes, mime } = await readAttachment(api, a.id);
  return previewOf(
    kindOf(mime || a.mime, a.name),
    { text: textOf(bytes), bytes: () => bytes, url: () => dataUrl(bytes, mime || a.mime) },
    (html) => inlinePage(html, a.name, null),
  );
}

/** What an attachment shows: an attached file's bytes, or a linked artifact. */
function loadPreview(
  api: BoardApi,
  a: ItemAttachment,
  readArtifact: Reader | null,
): Promise<Preview> {
  return a.kind === "doc" ? artifactPreview(a, readArtifact) : filePreview(api, a);
}

type State =
  | { readonly status: "ready"; readonly id: string; readonly preview: Preview }
  | { readonly status: "failed"; readonly id: string; readonly message: string };

/** The attachment, loaded and shown. */
export default function DocumentPreview(props: {
  readonly api: BoardApi;
  readonly attachment: ItemAttachment;
  readonly readArtifact: Reader | null;
}): ReactElement {
  const { api, attachment, readArtifact } = props;
  const [state, setState] = useState<State | null>(null);
  useEffect(() => {
    let live = true;
    const load = async (): Promise<void> => {
      try {
        const preview = await loadPreview(api, attachment, readArtifact);
        if (live) {
          setState({ status: "ready", id: attachment.id, preview });
        }
      } catch (failure) {
        if (live) {
          setState({ status: "failed", id: attachment.id, message: errText(failure) });
        }
      }
    };
    void load();
    return () => {
      live = false;
    };
  }, [api, attachment, readArtifact]);
  // Another attachment's answer means this one is still on its way.
  if (state === null || state.id !== attachment.id) {
    return <span className="lightbox-note">Loading…</span>;
  }
  if (state.status === "failed") {
    return <span className="lightbox-note">{state.message}</span>;
  }
  return <PreviewView preview={state.preview} title={attachment.name} />;
}

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
export function useArtifactReader(client: DaemonApi | undefined, projectId: string | undefined): Reader | null {
  return useMemo(() => {
    if (client === undefined || projectId === undefined) {
      return null;
    }
    return async (path: string) => {
      const reply = await client.request({ type: "read_file", project_id: projectId, path }, "file");
      return reply.file;
    };
  }, [client, projectId]);
}

/** An HTML page as the card's own: scripts run, nothing leaves it. */
export function SandboxedPage(props: { readonly page: string; readonly title: string }): ReactElement {
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
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    const blob = new Blob([props.bytes as BlobPart], { type: "application/pdf" });
    const next = URL.createObjectURL(blob);
    setUrl(next);
    return () => URL.revokeObjectURL(next);
  }, [props.bytes]);
  return url === null ? <span className="lightbox-note">Loading…</span> : (
    <iframe className="document-page" title={props.title} src={url} />
  );
}

export function PreviewView(props: { readonly preview: Preview; readonly title: string }): ReactElement {
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

/**
 * What an attachment shows: an attached file's bytes, or a linked
 * artifact read with `readArtifact` (its page's files beside it too).
 */
export async function loadPreview(
  api: BoardApi,
  a: ItemAttachment,
  readArtifact: Reader | null,
): Promise<Preview> {
  if (a.kind === "doc") {
    if (a.path === "") {
      return { kind: "none", why: "A web address: open it in your browser." };
    }
    if (readArtifact === null) {
      return { kind: "none", why: "Open this card on the computer that keeps its board to read it." };
    }
    const file = await readArtifact(a.path);
    const kind = kindOf(file.mime, a.path);
    if (kind === "html") {
      return { kind: "html", page: await inlinePage(file.text ?? "", a.path, readArtifact) };
    }
    if (kind === "markdown" || kind === "text") {
      return { kind, text: file.text ?? "" };
    }
    if (kind === "pdf" && file.base64 !== undefined) {
      return { kind: "pdf", bytes: Uint8Array.from(atob(file.base64), (c) => c.charCodeAt(0)) };
    }
    if (kind === "image" && file.base64 !== undefined) {
      return { kind: "image", url: `data:${file.mime};base64,${file.base64}` };
    }
    return { kind: "none", why: `No preview for this kind of file (${file.mime || a.path}).` };
  }
  const { bytes, mime } = await readAttachment(api, a.id);
  const kind = kindOf(mime || a.mime, a.name);
  switch (kind) {
    // An attached page is one file: what it links beside it isn't attached.
    case "html":
      return { kind: "html", page: await inlinePage(textOf(bytes), a.name, null) };
    case "markdown":
    case "text":
      return { kind, text: textOf(bytes) };
    case "pdf":
      return { kind: "pdf", bytes };
    case "image":
      return { kind: "image", url: dataUrl(bytes, mime || a.mime) };
    case null:
      return { kind: "none", why: "No preview for this kind of file." };
  }
}

type State =
  | { readonly status: "loading" }
  | { readonly status: "ready"; readonly preview: Preview }
  | { readonly status: "failed"; readonly message: string };

/** The attachment, loaded and shown. */
export default function DocumentPreview(props: {
  readonly api: BoardApi;
  readonly attachment: ItemAttachment;
  readonly readArtifact: Reader | null;
}): ReactElement {
  const { api, attachment, readArtifact } = props;
  const [state, setState] = useState<State>({ status: "loading" });
  useEffect(() => {
    let live = true;
    setState({ status: "loading" });
    loadPreview(api, attachment, readArtifact).then(
      (preview) => {
        if (live) {
          setState({ status: "ready", preview });
        }
      },
      (failure: unknown) => {
        if (live) {
          setState({ status: "failed", message: errText(failure) });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [api, attachment, readArtifact]);
  if (state.status === "loading") {
    return <span className="lightbox-note">Loading…</span>;
  }
  if (state.status === "failed") {
    return <span className="lightbox-note">{state.message}</span>;
  }
  return <PreviewView preview={state.preview} title={attachment.name} />;
}

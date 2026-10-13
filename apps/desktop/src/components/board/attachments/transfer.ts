// Moving an attachment's bytes (0.19.0): an upload is `attachment_upload`
// chunks of at most 512 KiB (the WebSocket frame cap is 1 MiB), then
// `item_attach{upload_id}`; reading one back is `attachment_read` chunks.

import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { ItemDetail } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { CHUNK } from "./attachments";

/** What an upload reads from: a `File` or anything else with a name. */
export interface Upload {
  readonly name: string;
  readonly size: number;
  readonly slice: (start: number, end: number) => Blob;
}

/** Sends `file` in chunks, reporting bytes received, then attaches it. */
export async function uploadAttachment(
  api: BoardApi,
  itemId: string,
  uploadId: string,
  file: Upload,
  onProgress: (received: number) => void,
  caption?: string,
): Promise<ItemDetail> {
  let offset = 0;
  // An empty file is refused before it gets here (`refusalFor`).
  do {
    const end = Math.min(offset + CHUNK, file.size);
    // Chunks go in order: each one's offset is what the home has received.
    // oxlint-disable-next-line no-await-in-loop
    const data = new Uint8Array(await file.slice(offset, end).arrayBuffer());
    // oxlint-disable-next-line no-await-in-loop
    const progress = await boardCall(
      api,
      {
        case: "attachmentUpload",
        value: {
          itemId,
          uploadId,
          offset: BigInt(offset),
          data,
          total: BigInt(file.size),
          name: file.name,
        },
      },
      "upload",
    );
    offset = Number(progress.received);
    onProgress(offset);
  } while (offset < file.size);
  return boardCall(
    api,
    { case: "itemAttach", value: { id: itemId, uploadId, name: file.name, caption } },
    "item",
  );
}

/** An attachment's whole bytes and type. */
export async function readAttachment(
  api: BoardApi,
  attachmentId: string,
): Promise<{ readonly bytes: Uint8Array; readonly mime: string }> {
  const parts: Uint8Array[] = [];
  let offset = 0n;
  let total = 0n;
  let mime = "";
  do {
    // Each read starts where the last one ended.
    // oxlint-disable-next-line no-await-in-loop
    const chunk = await boardCall(
      api,
      { case: "attachmentRead", value: { attachmentId, offset, limit: 0 } },
      "attachment",
    );
    if (chunk.data.length === 0) {
      break;
    }
    parts.push(chunk.data);
    offset += BigInt(chunk.data.length);
    total = chunk.total;
    mime = chunk.mime;
  } while (offset < total);
  const bytes = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const part of parts) {
    bytes.set(part, at);
    at += part.length;
  }
  return { bytes, mime };
}

/** A `data:` URL for an image's bytes. */
export function dataUrl(bytes: Uint8Array, mime: string): string {
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return `data:${mime || "application/octet-stream"};base64,${btoa(text)}`;
}

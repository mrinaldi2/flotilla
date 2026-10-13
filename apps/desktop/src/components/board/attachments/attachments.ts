// What an item may carry (0.19.0): the types and caps the Hermes service
// enforces, checked here first so a file is refused at once with its
// reason, and the words the drawer shows for each attachment.

import type { ItemAttachment } from "../../../protocol/gen/hermes/board/v1/board_pb";

const MB = 1024 * 1024;
/** At most this much data per `attachment_upload` chunk. */
export const CHUNK = 512 * 1024;

const IMAGES = new Set(["png", "jpg", "jpeg", "webp", "gif"]);
const FILES = new Set(["pdf", "md", "txt", "log"]);

/** The caps the Hermes service applies by default (kanban-rework.md, decision 6). */
export const CAPS = { image: 10 * MB, file: 25 * MB, item: 100 * MB } as const;

function extension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot < 0 ? "" : name.slice(dot + 1).toLowerCase();
}

export function isImageName(name: string): boolean {
  return IMAGES.has(extension(name));
}

/** "1.2 MB", "640 KB", "12 bytes". */
export function sizeWords(bytes: number): string {
  if (bytes >= MB) {
    return `${(bytes / MB).toFixed(1)} MB`;
  }
  if (bytes >= 1024) {
    return `${Math.round(bytes / 1024)} KB`;
  }
  return `${bytes} ${bytes === 1 ? "byte" : "bytes"}`;
}

/**
 * Why a file can't be attached, or null: its type, its own size, or the
 * item's total. `onItem` is what the item's attached files hold already.
 */
export function refusalFor(name: string, bytes: number, onItem: number): string | null {
  const ext = extension(name);
  if (ext === "heic" || ext === "heif") {
    return "HEIC isn't taken: export it as JPEG and attach that.";
  }
  const image = IMAGES.has(ext);
  if (!image && !FILES.has(ext)) {
    return `${ext === "" ? "A file without a type" : `.${ext}`} isn't taken: attach png, jpg, webp, gif, pdf, md, txt or log.`;
  }
  const cap = image ? CAPS.image : CAPS.file;
  if (bytes > cap) {
    return `${sizeWords(bytes)} is over the ${sizeWords(cap)} cap for ${image ? "an image" : "a file"}.`;
  }
  if (onItem + bytes > CAPS.item) {
    return `The item would hold ${sizeWords(onItem + bytes)}, over its ${sizeWords(CAPS.item)} cap.`;
  }
  return null;
}

/** What the item's attached files hold, detached ones and docs left out. */
export function attachedBytes(attachments: readonly ItemAttachment[]): number {
  return attachments
    .filter((a) => !a.removed && a.kind !== "doc")
    .reduce((sum, a) => sum + Number(a.bytes), 0);
}

/** The glyph and word for an attachment's kind: never colour alone. */
export function kindLabel(attachment: ItemAttachment): {
  readonly glyph: string;
  readonly word: string;
} {
  if (attachment.kind === "image") {
    return { glyph: "▣", word: "image" };
  }
  if (attachment.kind === "doc") {
    return attachment.url === "" ? { glyph: "❏", word: "document" } : { glyph: "↗", word: "link" };
  }
  const ext = extension(attachment.name);
  return { glyph: "▤", word: ext === "" ? "file" : ext };
}

/** The ids `![…](attachment:<id>)` refers to in a description. */
export function referencedIds(markdown: string): string[] {
  return [...markdown.matchAll(/\(attachment:([\w-]+)\)/g)].map((m) => m[1] ?? "");
}

/** The id an image's `src` names, or null when it isn't an attachment. */
export function attachmentId(src: string | undefined): string | null {
  return src?.startsWith("attachment:") === true ? src.slice("attachment:".length) : null;
}

/** The markdown that shows an attached image inline. */
export function inlineImage(attachment: ItemAttachment): string {
  const alt = (attachment.caption || attachment.name).replace(/[[\]]/g, "");
  return `![${alt}](attachment:${attachment.id})`;
}

/** An `upload_id`: letters, digits, '-' and '_'. */
export function newUploadId(now: number, seed: number): string {
  return `up-${now.toString(36)}-${Math.floor(seed * 1e9).toString(36)}`;
}

/** A pasted or dropped document: a project artifact path or a URL. */
export function docSource(
  text: string,
): { readonly url: string } | { readonly artifact: string } | null {
  const value = text.trim();
  if (value === "") {
    return null;
  }
  return /^https?:\/\//i.test(value) ? { url: value } : { artifact: value.replace(/^\/+/, "") };
}

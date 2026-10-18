// The plan (0.21.0, #43): goals, initiatives, epics and features over the
// board's requests, and the capabilities that gate each 0.21 addition. A
// document is read from the project's repository in chunks, and turned into
// the file shape the document preview reads.

import type { BoardApi } from "./board";
import { boardCall } from "./board";
import type { PlanDocContent } from "./gen/flotilla/board/v1/plan_pb";

/** The plan's requests and pushes (`plan.proto`). */
export const PLAN_V1 = "plan_v1";
/** Task and Design cards; Epic, Feature and Chore refused (`new.type_retired`). */
export const ITEM_TYPES_V2 = "item_types_v2";
/** The Research column, its `start.research` guard and `LINK_KIND_DESIGN`. */
export const COLUMN_RESEARCH = "column_research";

/** A repository file as the document preview reads it: text, or base64 for the rest. */
export interface PlanFile {
  readonly mime: string;
  readonly text?: string;
  readonly base64?: string;
  /** The commit it was read at. */
  readonly sha: string;
  /** With `diffFrom`: the unified diff from that commit. */
  readonly diff?: string;
}

/** Whether the bytes of `mime` read as text. */
export function isTextMime(mime: string): boolean {
  return (
    mime.startsWith("text/") ||
    mime === "application/json" ||
    mime.endsWith("+xml") ||
    mime.endsWith("+json")
  );
}

function joined(chunks: readonly Uint8Array[]): Uint8Array {
  const all = new Uint8Array(chunks.reduce((n, c) => n + c.length, 0));
  let at = 0;
  for (const chunk of chunks) {
    all.set(chunk, at);
    at += chunk.length;
  }
  return all;
}

function base64Of(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

/** The file from its chunks, as text or base64 by its MIME type. */
export function planFileOf(first: PlanDocContent, chunks: readonly Uint8Array[]): PlanFile {
  const bytes = joined(chunks);
  const body = isTextMime(first.mime)
    ? { text: new TextDecoder().decode(bytes) }
    : { base64: base64Of(bytes) };
  return {
    mime: first.mime,
    sha: first.sha,
    ...body,
    ...(first.diff === undefined ? {} : { diff: first.diff }),
  };
}

/**
 * Reads `path` of the project's repository at `sha` (the default branch's
 * head without one), every chunk of it; with `diffFrom`, also the diff from
 * that commit.
 */
export async function readPlanDoc(
  api: BoardApi,
  projectId: string,
  path: string,
  opts: { readonly sha?: string; readonly diffFrom?: string } = {},
): Promise<PlanFile> {
  const read = (offset: bigint, sha: string | undefined): Promise<PlanDocContent> =>
    boardCall(
      api,
      {
        case: "planDocRead",
        value: {
          projectId,
          path,
          sha,
          diffFrom: offset === 0n ? opts.diffFrom : undefined,
          offset,
        },
      },
      "planDoc",
    );
  const first = await read(0n, opts.sha);
  // The rest at the commit the first chunk was read at, so the file is one
  // version; each chunk starts where the last ended.
  const rest = async (offset: bigint): Promise<Uint8Array[]> => {
    if (offset >= first.total) {
      return [];
    }
    const next = await read(offset, first.sha);
    if (next.data.length === 0) {
      return [];
    }
    return [next.data, ...(await rest(offset + BigInt(next.data.length)))];
  };
  return planFileOf(first, [first.data, ...(await rest(BigInt(first.data.length)))]);
}

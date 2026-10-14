// Files on their way to an item (0.19.0): each is checked against the caps
// at once, so a refused one shows its reason without sending a byte; the
// others upload in chunks with their progress, then attach. One on its way
// can be cancelled, and closing the drawer cancels them all.

import { useCallback, useEffect, useRef, useState } from "react";
import type { BoardApi } from "../../../protocol/board";
import type { ItemAttachment } from "../../../protocol/gen/hermes/board/v1/board_pb";
import type { ItemDetail } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { errText } from "../../../util";
import { attachedBytes, newUploadId, refusalFor } from "./attachments";
import type { Upload } from "./transfer";
import { UploadCancelled, uploadAttachment } from "./transfer";

export interface UploadState {
  readonly id: string;
  readonly name: string;
  readonly size: number;
  readonly received: number;
  /** Why it was refused, by the caps here or by the Hermes service. */
  readonly refusal: string | null;
}

export interface Uploads {
  readonly uploads: readonly UploadState[];
  /** Uploads each file; resolves with the attachments added, in order. */
  readonly add: (files: readonly Upload[]) => Promise<ItemAttachment[]>;
  /** Drops a refused one from the list, or cancels one on its way. */
  readonly dismiss: (id: string) => void;
}

export function useUploads(
  api: BoardApi,
  itemId: string,
  attachments: readonly ItemAttachment[],
  onDetail: (detail: ItemDetail) => void,
): Uploads {
  const [uploads, setUploads] = useState<readonly UploadState[]>([]);
  // The uploads on their way, by id, to cancel them.
  const sending = useRef(new Map<string, AbortController>());
  useEffect(() => {
    const all = sending.current;
    return () => {
      for (const controller of all.values()) {
        controller.abort();
      }
    };
  }, []);
  const patch = useCallback((id: string, change: Partial<UploadState>): void => {
    setUploads((all) => all.map((u) => (u.id === id ? { ...u, ...change } : u)));
  }, []);

  const add = useCallback(
    async (files: readonly Upload[]): Promise<ItemAttachment[]> => {
      const added: ItemAttachment[] = [];
      let onItem = attachedBytes(attachments);
      for (const file of files) {
        const id = newUploadId(Date.now(), Math.random());
        const refusal = refusalFor(file.name, file.size, onItem);
        setUploads((all) => [
          ...all,
          { id, name: file.name, size: file.size, received: 0, refusal },
        ]);
        if (refusal !== null) {
          continue;
        }
        const controller = new AbortController();
        sending.current.set(id, controller);
        try {
          const before = new Set(attachments.map((a) => a.id));
          // One file at a time, so the item's cap is checked against what landed.
          // oxlint-disable-next-line no-await-in-loop
          const detail = await uploadAttachment(
            api,
            itemId,
            id,
            file,
            (received) => patch(id, { received }),
            { signal: controller.signal },
          );
          onItem += file.size;
          onDetail(detail);
          setUploads((all) => all.filter((u) => u.id !== id));
          const fresh = detail.attachments.filter((a) => !before.has(a.id) && !a.removed);
          added.push(...fresh.slice(-1));
        } catch (failure) {
          if (!(failure instanceof UploadCancelled)) {
            patch(id, { refusal: errText(failure) });
          }
        } finally {
          sending.current.delete(id);
        }
      }
      return added;
    },
    [api, itemId, attachments, onDetail, patch],
  );

  const dismiss = useCallback((id: string): void => {
    sending.current.get(id)?.abort();
    setUploads((all) => all.filter((u) => u.id !== id));
  }, []);

  return { uploads, add, dismiss };
}

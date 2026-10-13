// What the item drawer does with attachments (0.19.0): files from "+ Add
// file", a drop on the drawer or a paste into the description; a linked
// document; detaching. Each answer is the item as it is now.

import { useCallback, useMemo } from "react";
import type { DragEvent } from "react";
import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { ItemAttachment } from "../../../protocol/gen/hermes/board/v1/board_pb";
import type { ItemDetail } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { DaemonError } from "../../../protocol/connection";
import { docSource, inlineImage } from "./attachments";
import type { AttachmentActions } from "./AttachmentsSection";
import type { UploadState } from "./useUploads";
import { useUploads } from "./useUploads";

export interface DrawerAttachments {
  readonly uploads: readonly UploadState[];
  readonly actions: AttachmentActions;
  /** Attaches a pasted image and gives the Markdown that shows it inline. */
  readonly pasteImage: (file: File) => Promise<string | null>;
  readonly dropTarget: {
    readonly onDragOver: (event: DragEvent<HTMLElement>) => void;
    readonly onDrop: (event: DragEvent<HTMLElement>) => void;
  };
}

function hasFiles(event: DragEvent<HTMLElement>): boolean {
  return [...event.dataTransfer.types].includes("Files");
}

export function useDrawerAttachments(
  api: BoardApi,
  itemId: string,
  attachments: readonly ItemAttachment[],
  setDetail: (detail: ItemDetail) => void,
): DrawerAttachments {
  const { uploads, add, dismiss } = useUploads(api, itemId, attachments, setDetail);

  const onLink = useCallback(
    async (text: string): Promise<void> => {
      const source = docSource(text);
      if (source === null) {
        throw new DaemonError("invalid_request", "Give an artifact path or a URL.");
      }
      setDetail(
        await boardCall(api, { case: "itemAttach", value: { id: itemId, ...source } }, "item"),
      );
    },
    [api, itemId, setDetail],
  );
  const onDetach = useCallback(
    (attachment: ItemAttachment): void => {
      boardCall(
        api,
        { case: "itemDetach", value: { id: itemId, attachmentId: attachment.id } },
        "item",
      ).then(setDetail, () => {
        // Still attached: the list shows it as it was.
      });
    },
    [api, itemId, setDetail],
  );
  const pasteImage = useCallback(
    async (file: File): Promise<string | null> => {
      const [added] = await add([file]);
      return added === undefined ? null : inlineImage(added);
    },
    [add],
  );

  return useMemo(
    () => ({
      uploads,
      actions: {
        onAddFiles: (files) => void add(files),
        onLink,
        onDetach,
        onDismiss: dismiss,
      },
      pasteImage,
      dropTarget: {
        onDragOver: (event) => {
          if (hasFiles(event)) {
            event.preventDefault();
          }
        },
        onDrop: (event) => {
          if (hasFiles(event)) {
            event.preventDefault();
            void add([...event.dataTransfer.files]);
          }
        },
      },
    }),
    [uploads, add, onLink, onDetach, dismiss, pasteImage],
  );
}

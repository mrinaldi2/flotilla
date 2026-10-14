// An attached image, small (the list) or inline (the description): a
// button that opens it full size. While its bytes load, or when they
// can't, it says so in words; a detached one is a placeholder.

import type { ReactElement } from "react";
import type { BoardApi } from "../../../protocol/board";
import type { ItemAttachment } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { useAttachmentImage } from "./useAttachmentImage";

interface AttachmentThumbProps {
  readonly api: BoardApi;
  readonly attachment: ItemAttachment;
  readonly onOpen: (attachment: ItemAttachment) => void;
  /** In the description: as wide as the text allows. */
  readonly inline?: boolean;
  /** The description's alt text, when it gives one. */
  readonly alt?: string;
}

/** What shows for an `attachment:` the item doesn't carry (any more). */
export function MissingImage({ name }: { readonly name: string }): ReactElement {
  return (
    <span className="attachment-missing" role="img" aria-label={`${name}: removed`}>
      <span aria-hidden="true">⊘</span> {name} · removed
    </span>
  );
}

export default function AttachmentThumb(props: AttachmentThumbProps): ReactElement {
  const { attachment, inline } = props;
  const image = useAttachmentImage(props.api, attachment.removed ? null : attachment.id);
  const alt = props.alt || attachment.caption || attachment.name;
  if (attachment.removed) {
    return <MissingImage name={alt} />;
  }
  return (
    <button
      type="button"
      className={inline === true ? "attachment-inline" : "attachment-thumb"}
      aria-label={`Open ${alt} full size`}
      onClick={() => props.onOpen(attachment)}
    >
      {image.kind === "ready" ? (
        <img src={image.url} alt={alt} />
      ) : (
        <span className="attachment-thumb-note">
          {image.kind === "loading" ? "Loading…" : "Can't show it"}
        </span>
      )}
    </button>
  );
}

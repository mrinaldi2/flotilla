// An attachment full size (0.19.0): an image fitted to the window, or a
// file's or document's details. Esc or the backdrop closes it.

import { X } from "lucide-react";
import type { ReactElement } from "react";
import { useEscapeKey } from "../../../hooks/useEscapeKey";
import type { BoardApi } from "../../../protocol/board";
import type { ItemAttachment } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { kindLabel, sizeWords } from "./attachments";
import { useAttachmentImage } from "./useAttachmentImage";

interface AttachmentViewerProps {
  readonly api: BoardApi;
  readonly attachment: ItemAttachment;
  readonly onClose: () => void;
}

export default function AttachmentViewer(props: AttachmentViewerProps): ReactElement {
  const { attachment: a, onClose } = props;
  const image = useAttachmentImage(props.api, a.kind === "image" && !a.removed ? a.id : null);
  useEscapeKey(onClose);
  const label = kindLabel(a);
  const size = a.kind === "doc" ? "" : ` · ${sizeWords(Number(a.bytes))}`;
  return (
    <div
      className="lightbox"
      role="presentation"
      onMouseDown={(event) => {
        if (event.target instanceof HTMLElement && event.target.dataset.backdrop === "true") {
          onClose();
        }
      }}
    >
      <div
        className="lightbox-dialog"
        role="dialog"
        aria-modal="true"
        aria-label={a.name}
        data-backdrop="true"
      >
        <div className="lightbox-bar" data-backdrop="true">
          <span className="lightbox-count">{`${label.glyph} ${a.name}`}</span>
          <span className="lightbox-hint">{`${label.word}${size}`}</span>
          <button type="button" className="icon-btn" aria-label="Close" autoFocus onClick={onClose}>
            <X size={18} aria-hidden="true" />
          </button>
        </div>
        <div className="lightbox-stage" data-backdrop="true">
          {a.kind !== "image" ? (
            <div className="attachment-details">
              {a.caption ? <p>{a.caption}</p> : null}
              <p className="mono">{a.url || a.path}</p>
              <p className="drawer-dim">
                {a.kind === "doc"
                  ? "A linked document: bots read it from the project's artifacts or its address."
                  : "Kept in the project's artifacts on the board's home; bots read it there."}
              </p>
            </div>
          ) : image.kind === "ready" ? (
            <span className="lightbox-image">
              <img src={image.url} alt={a.caption || a.name} />
            </span>
          ) : (
            <span className="lightbox-note">
              {image.kind === "loading" ? "Loading image…" : "Image unavailable"}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}

// An item's description (0.19.0): Markdown as bots and the owner write it,
// with `![caption](attachment:<id>)` drawn as the item's own image. Its
// bytes come from the board's home, so it is safe to show; an image that
// is not attached, or was detached, is a placeholder, and a file or a
// document named that way is a button that opens it. Every other image
// and link stays inert text, as in any bot-written Markdown.

import type { ReactElement } from "react";
import ReactMarkdown from "react-markdown";
import type { BoardApi } from "../../../protocol/board";
import type { ItemAttachment } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { useCardMarkdown } from "../../cards/cardMarkdown";
import { SAFE_LINKS } from "../../control/Markdown";
import { attachmentId, kindLabel } from "./attachments";
import AttachmentThumb, { MissingImage } from "./AttachmentThumb";

interface ItemMarkdownProps {
  readonly api: BoardApi;
  readonly text: string;
  readonly attachments: readonly ItemAttachment[];
  readonly onOpen: (attachment: ItemAttachment) => void;
}

export default function ItemMarkdown(props: ItemMarkdownProps): ReactElement {
  const cards = useCardMarkdown(SAFE_LINKS.a);
  const img = ({ alt, src }: { alt?: string; src?: string }): ReactElement => {
    const id = attachmentId(src);
    if (id === null) {
      return SAFE_LINKS.img({ alt, src });
    }
    const found = props.attachments.find((a) => a.id === id);
    if (found === undefined) {
      return <MissingImage name={alt || "image"} />;
    }
    if (found.kind !== "image" && !found.removed) {
      // A file or a document: named, never read as an image.
      const label = kindLabel(found);
      return (
        <button
          type="button"
          className="chat-link attachment-inline-file"
          aria-label={`Open ${found.name}`}
          onClick={() => props.onOpen(found)}
        >
          <span aria-hidden="true">{label.glyph} </span>
          {`${found.name} · ${label.word}`}
        </button>
      );
    }
    return (
      <AttachmentThumb api={props.api} attachment={found} alt={alt} inline onOpen={props.onOpen} />
    );
  };
  return (
    <div className="markdown">
      <ReactMarkdown
        components={{ ...SAFE_LINKS, a: cards.a, img }}
        remarkPlugins={cards.remarkPlugins}
        urlTransform={(url) => (attachmentId(url) === null ? cards.urlTransform(url) : url)}
      >
        {props.text}
      </ReactMarkdown>
    </div>
  );
}

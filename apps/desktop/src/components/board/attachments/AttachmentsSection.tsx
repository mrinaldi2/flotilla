// An item's attachments in its drawer (0.19.0): images as thumbnails that
// open full size, files with their type and size, linked documents, the
// uploads on their way with their progress or refusal, and, for the owner,
// "+ Add file", "+ Link document" and Detach.

import { useRef, useState } from "react";
import type { FormEvent, ReactElement } from "react";
import type { BoardApi } from "../../../protocol/board";
import type { ItemAttachment } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { kindLabel, sizeWords } from "./attachments";
import AttachmentThumb from "./AttachmentThumb";
import type { UploadState } from "./useUploads";

export interface AttachmentActions {
  readonly onAddFiles: (files: readonly File[]) => void;
  /** A project artifact path or a URL. */
  readonly onLink: (text: string) => Promise<void>;
  readonly onDetach: (attachment: ItemAttachment) => void;
  /** Drops a refused upload from the list, or cancels one on its way. */
  readonly onDismiss: (uploadId: string) => void;
}

interface AttachmentsSectionProps {
  readonly api: BoardApi;
  readonly attachments: readonly ItemAttachment[];
  readonly uploads: readonly UploadState[];
  /** Absent for a connection that can't change the item. */
  readonly actions?: AttachmentActions;
  readonly onOpen: (attachment: ItemAttachment) => void;
}

function LinkForm(props: {
  readonly onLink: (text: string) => Promise<void>;
  readonly onDone: () => void;
}): ReactElement {
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const submit = (event: FormEvent): void => {
    event.preventDefault();
    props
      .onLink(text)
      .then(props.onDone, (failure: unknown) =>
        setError(failure instanceof Error ? failure.message : String(failure)),
      );
  };
  return (
    <form className="attachment-link-form" onSubmit={submit}>
      <input
        type="text"
        aria-label="Artifact path or URL"
        placeholder="specs/import.md or https://…"
        value={text}
        autoFocus
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(event) => {
          // Esc closes this form, not the drawer it is in.
          if (event.key === "Escape") {
            event.preventDefault();
            props.onDone();
          }
        }}
      />
      <button type="submit" className="btn btn-small" disabled={text.trim() === ""}>
        Link
      </button>
      <button type="button" className="btn btn-small" onClick={props.onDone}>
        Cancel
      </button>
      {error === null ? null : (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

function Row(props: {
  readonly api: BoardApi;
  readonly attachment: ItemAttachment;
  readonly onOpen: (attachment: ItemAttachment) => void;
  readonly onDetach?: (attachment: ItemAttachment) => void;
}): ReactElement {
  const { attachment: a } = props;
  const name = a.caption ? `${a.name} · ${a.caption}` : a.name;
  return (
    <li className="attachment-row">
      {a.kind === "image" ? (
        <AttachmentThumb api={props.api} attachment={a} onOpen={props.onOpen} />
      ) : (
        <span className="attachment-icon" aria-hidden="true">
          {kindLabel(a).glyph}
        </span>
      )}
      <span className="attachment-text">
        {a.kind === "image" ? (
          <span className="attachment-name">{name}</span>
        ) : (
          <button
            type="button"
            className="attachment-name chat-link"
            onClick={() => props.onOpen(a)}
          >
            {name}
          </button>
        )}
        <span className="attachment-meta">
          {[kindLabel(a).word, a.kind === "doc" ? a.url || a.path : sizeWords(Number(a.bytes))]
            .filter(Boolean)
            .join(" · ")}
        </span>
      </span>
      {props.onDetach === undefined ? null : (
        <button
          type="button"
          className="btn btn-small attachment-detach"
          aria-label={`Detach ${a.name}`}
          onClick={() => props.onDetach?.(a)}
        >
          Detach
        </button>
      )}
    </li>
  );
}

export default function AttachmentsSection(props: AttachmentsSectionProps): ReactElement {
  const { attachments, uploads, actions } = props;
  const [linking, setLinking] = useState(false);
  const input = useRef<HTMLInputElement | null>(null);
  const attached = attachments.filter((a) => !a.removed);
  const detached = attachments.filter((a) => a.removed);
  return (
    <section className="drawer-attachments" aria-label="Attachments">
      <h4>Attachments {attached.length > 0 ? attached.length : ""}</h4>
      {attached.length === 0 && uploads.length === 0 ? (
        <p className="drawer-empty">
          {actions === undefined ? "None." : "None yet. Drop a file on the drawer, or add one."}
        </p>
      ) : null}
      <ul className="attachment-list">
        {attached.map((a) => (
          <Row
            key={a.id}
            api={props.api}
            attachment={a}
            onOpen={props.onOpen}
            onDetach={actions?.onDetach}
          />
        ))}
        {uploads.map((u) => (
          <li
            key={u.id}
            className={`attachment-row${u.refusal === null ? "" : " attachment-refused"}`}
          >
            <span className="attachment-icon" aria-hidden="true">
              {u.refusal === null ? "↑" : "✗"}
            </span>
            <span className="attachment-text">
              <span className="attachment-name">{u.name}</span>
              {u.refusal === null ? (
                <span className="attachment-meta">
                  <progress
                    max={u.size || 1}
                    value={u.received}
                    aria-label={`Uploading ${u.name}`}
                  />{" "}
                  {`${sizeWords(u.received)} of ${sizeWords(u.size)}`}
                </span>
              ) : (
                <span className="attachment-meta" role="alert">
                  Refused: {u.refusal}
                </span>
              )}
            </span>
            {actions === undefined ? null : (
              <button
                type="button"
                className="btn btn-small"
                aria-label={`${u.refusal === null ? "Cancel uploading" : "Dismiss"} ${u.name}`}
                onClick={() => actions.onDismiss(u.id)}
              >
                {u.refusal === null ? "Cancel" : "Dismiss"}
              </button>
            )}
          </li>
        ))}
      </ul>
      {detached.length === 0 ? null : (
        <p className="attachment-detached">
          <span aria-hidden="true">⊘</span> Detached, kept in the history:{" "}
          {detached.map((a) => a.name).join(", ")}
        </p>
      )}
      {actions === undefined ? null : linking ? (
        <LinkForm onLink={actions.onLink} onDone={() => setLinking(false)} />
      ) : (
        <div className="attachment-actions">
          <button type="button" className="btn btn-small" onClick={() => input.current?.click()}>
            + Add file
          </button>
          <button type="button" className="btn btn-small" onClick={() => setLinking(true)}>
            + Link document
          </button>
          <input
            ref={input}
            type="file"
            multiple
            hidden
            aria-label="Add files"
            accept=".png,.jpg,.jpeg,.webp,.gif,.pdf,.md,.txt,.log"
            onChange={(e) => {
              actions.onAddFiles([...(e.target.files ?? [])]);
              e.target.value = "";
            }}
          />
        </div>
      )}
    </section>
  );
}

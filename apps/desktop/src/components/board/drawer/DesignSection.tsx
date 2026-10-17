// A task's design (0.21.0, #43): the done Design cards linked to it
// (`LINK_KIND_DESIGN`), each with its documents, so the bot building it
// reads what was designed. On a done Design card, the owner attaches it to
// the task that builds it.

import { useEffect, useState } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { ItemCard, ItemLink } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { ItemType, LinkKind } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { ItemDetail } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { errText } from "../../../util";
import { typeLabel } from "../labels";

/** "design.md · timing-curve.html · 3 more": a design card's documents in one line. */
export function documentsLine(detail: ItemDetail | null): string {
  const names = (detail?.attachments ?? []).map((a) => a.name);
  if (names.length === 0) {
    return "No documents yet";
  }
  const shown = names.slice(0, 2).join(" · ");
  return names.length > 2 ? `${shown} · ${names.length - 2} more` : shown;
}

function DesignLink(props: {
  readonly api: BoardApi;
  readonly link: ItemLink;
  readonly columnName: (key: string) => string;
  readonly onOpenItem?: (id: string) => void;
}): ReactElement {
  const { api, link, onOpenItem } = props;
  const [detail, setDetail] = useState<ItemDetail | null>(null);
  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const read = await boardCall(api, { case: "itemGet", value: { id: link.ref } }, "item");
        if (live) {
          setDetail(read);
        }
      } catch {
        // The card stays named by its id and label.
      }
    })();
    return () => {
      live = false;
    };
  }, [api, link.ref]);
  const item = detail?.item;
  const title = item?.title ?? link.label ?? "";
  return (
    <li className="drawer-design">
      <span className="drawer-design-glyph" aria-hidden="true">
        {typeLabel(ItemType.DESIGN).glyph}
      </span>
      <span className="drawer-design-text">
        <span className="drawer-design-title">
          <span className="mono">{link.ref}</span> {title}
          {item === undefined ? "" : ` · ${props.columnName(item.columnKey)}`}
        </span>
        <span className="drawer-dim">{documentsLine(detail)}</span>
      </span>
      {onOpenItem === undefined ? null : (
        <button
          type="button"
          className="btn btn-small"
          aria-label={`Read ${link.ref}`}
          onClick={() => onOpenItem(link.ref)}
        >
          Read
        </button>
      )}
    </li>
  );
}

/** The design cards a task builds; nothing when it builds none. */
export function DesignSection(props: {
  readonly api: BoardApi;
  readonly links: readonly ItemLink[];
  readonly columnName: (key: string) => string;
  readonly onOpenItem?: (id: string) => void;
}): ReactElement | null {
  const designs = props.links.filter((l) => l.kind === LinkKind.DESIGN);
  if (designs.length === 0) {
    return null;
  }
  return (
    <section className="drawer-designs" aria-label="Design">
      <h4>Design</h4>
      <ul>
        {designs.map((link) => (
          <DesignLink
            key={link.ref}
            api={props.api}
            link={link}
            columnName={props.columnName}
            onOpenItem={props.onOpenItem}
          />
        ))}
      </ul>
    </section>
  );
}

/** "Attach design to a task…": links this done Design card to a task on its board. */
export function AttachDesign(props: {
  readonly api: BoardApi;
  readonly designId: string;
  /** The board's tasks that aren't done. */
  readonly tasks: readonly ItemCard[];
}): ReactElement | null {
  const [open, setOpen] = useState(false);
  const [task, setTask] = useState<string>("");
  const [sending, setSending] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  if (props.tasks.length === 0) {
    return null;
  }
  const chosen = task === "" ? props.tasks[0]?.id : task;
  const attach = (): void => {
    if (chosen === undefined || sending) {
      return;
    }
    setSending(true);
    setSaid(null);
    const value = { id: chosen, kind: LinkKind.DESIGN, ref: props.designId };
    void (async () => {
      try {
        const result = await boardCall(props.api, { case: "itemLink", value }, "edited");
        setSaid(
          result.outcome.case === "refused"
            ? result.outcome.value.unmet.map((u) => u.text).join(" ")
            : `Attached to ${chosen}: it shows this design's documents.`,
        );
        if (result.outcome.case === "done") {
          setOpen(false);
        }
      } catch (failure) {
        setSaid(errText(failure));
      } finally {
        setSending(false);
      }
    })();
  };
  return (
    <section className="drawer-attach-design" aria-label="Attach design">
      {open ? (
        <div className="drawer-attach-row">
          <label className="field">
            <span className="field-label">The task that builds it</span>
            <select value={chosen ?? ""} onChange={(e) => setTask(e.target.value)}>
              {props.tasks.map((t) => (
                <option key={t.id} value={t.id}>{`${t.id} · ${t.title}`}</option>
              ))}
            </select>
          </label>
          <button
            type="button"
            className="btn btn-small btn-primary"
            disabled={sending}
            onClick={attach}
          >
            {sending ? "Attaching…" : "Attach"}
          </button>
          <button type="button" className="btn btn-small" onClick={() => setOpen(false)}>
            Cancel
          </button>
        </div>
      ) : (
        <button
          type="button"
          className="btn btn-small drawer-design-btn"
          onClick={() => setOpen(true)}
        >
          Attach design to a task…
        </button>
      )}
      {said === null ? null : (
        <p className="drawer-dim" role="status">
          {said}
        </p>
      )}
    </section>
  );
}

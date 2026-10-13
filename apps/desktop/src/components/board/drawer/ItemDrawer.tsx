// The item drawer (U4, H-018 §3.6): a 440px right drawer over the board or
// the dashboard. The state stepper, a meta line, the next step as the guard
// check turns it into guidance, then Overview, Links and Activity. Esc
// closes it; ↑/↓ move to the neighbouring card when the board offers them.
// Focus lands on the title once the item loads, so a screen reader reads it
// (UX-012); the caller returns it on close.
//
// 0.19.0: the owner edits the item here, and attaches files (a drop on the
// drawer, a paste into the description, "+ Add file") and documents. A
// draft is headed "Draft · not on the board yet", with Discuss with a bot,
// its changes to review, and Discard / Finalize at the foot.

import { useEffect, useRef, useState } from "react";
import type { ReactElement, ReactNode } from "react";
import { escapeIsTheDrawers } from "../../../hooks/useDrawerEscape";
import type { DaemonApi } from "../../../protocol/api";
import type { BoardApi } from "../../../protocol/board";
import type { Bot } from "../../../protocol/entities";
import type { BoardColumn, ItemAttachment } from "../../../protocol/gen/hermes/board/v1/board_pb";
import AttachmentsSection from "../attachments/AttachmentsSection";
import AttachmentViewer from "../attachments/AttachmentViewer";
import ItemMarkdown from "../attachments/ItemMarkdown";
import { useDrawerAttachments } from "../attachments/useDrawerAttachments";
import { useDraftDrawer } from "../drafts/useDraftDrawer";
import ItemEditForm from "../edit/ItemEditForm";
import type { Anchor } from "../useBoardMoves";
import { DrawerHeader, MetaLine, NextStepView, Stepper } from "./DrawerParts";
import { DrawerTabs } from "./DrawerTabsView";
import type { Tab } from "./DrawerTabsView";
import { nextStep } from "./drawerText";
import { useItemDetail } from "./useItemDetail";

export interface ItemDrawerProps {
  readonly api: BoardApi;
  readonly itemId: string;
  /** The card's version on the board: a push that moves it reads again. */
  readonly version?: bigint;
  readonly columns: readonly BoardColumn[];
  readonly bots: readonly Bot[];
  /** Whether this connection may comment (the control grant). */
  readonly canComment: boolean;
  /** Whether it may edit the item and its attachments; as `canComment` when unset. */
  readonly canEdit?: boolean;
  readonly onClose: () => void;
  /** Opens "Move to…" below the button; absent where moves aren't offered. */
  readonly onMove?: (anchor: Anchor) => void;
  /** Steps to the previous (-1) or next (1) card in the column. */
  readonly onStep?: (direction: -1 | 1) => void;
  /** The clock, for "Doing for 31h"; tests and stories pin it. */
  readonly now?: () => number;
  /** The tab it opens on; Overview unless a story shows another. */
  readonly initialTab?: Tab;
  /** Shown above the tabs: the item's commands for the owner (H-117). */
  readonly children?: ReactNode;
  /** The card shown before, for `‹ Back` (UX-035 §4). */
  readonly back?: { readonly id: string; readonly onBack: () => void };
  /** Below everything (a `drawer-foot`): "Open on the board" off the board. */
  readonly footer?: ReactNode;
  /** For a draft's docked chat ("Discuss with a bot"). */
  readonly client?: DaemonApi;
  readonly connected?: boolean;
  /** The project's lead, first in "Discuss with a bot". */
  readonly leadId?: string;
  /** A draft was finalized or discarded: what happened, in words. */
  readonly onDraftDone?: (message: string) => void;
  /** Stories: open in the edit form, or with a bot's chat docked. */
  readonly initialEdit?: boolean;
  readonly initialDiscuss?: string;
}

/** "bot:<id>", "user", "device:…": who did something, in words. */
function whoNamer(bots: readonly Bot[]): (actor: string) => string {
  return (actor) => {
    if (actor === "user" || actor === "owner" || actor.startsWith("device:")) {
      return "You";
    }
    const id = actor.startsWith("bot:") ? actor.slice(4) : actor;
    return bots.find((b) => b.id === id)?.name ?? "a bot";
  };
}

function useDrawerKeys(onClose: () => void, onStep?: (direction: -1 | 1) => void): void {
  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      const typing =
        event.target instanceof HTMLElement &&
        (event.target.tagName === "TEXTAREA" ||
          event.target.tagName === "INPUT" ||
          event.target.tagName === "SELECT");
      if (event.key === "Escape") {
        // A menu or dialog over the drawer takes its own Esc first.
        if (escapeIsTheDrawers(event)) {
          onClose();
        }
      } else if (!typing && onStep && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
        event.preventDefault();
        onStep(event.key === "ArrowUp" ? -1 : 1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, onStep]);
}

export default function ItemDrawer(props: ItemDrawerProps): ReactElement {
  const { api, itemId, columns, bots } = props;
  const { detail, check, error, comment, setDetail, reload } = useItemDetail(
    api,
    itemId,
    props.version,
  );
  const [editing, setEditing] = useState(props.initialEdit === true);
  const [viewing, setViewing] = useState<ItemAttachment | null>(null);
  const editable = props.canEdit ?? props.canComment;
  const who = whoNamer(bots);
  const columnName = (key: string): string => columns.find((c) => c.key === key)?.name ?? key;
  const item = detail?.item;
  const attachments = detail?.attachments ?? [];
  const files = useDrawerAttachments(api, itemId, attachments, setDetail);
  const now = props.now ?? Date.now;
  const draft = useDraftDrawer({
    api,
    client: editable ? props.client : undefined,
    itemId,
    item,
    detail,
    bots,
    leadId: props.leadId,
    columns: columns.filter((c) => c.boardId === "" || c.boardId === item?.boardId),
    who,
    connected: props.connected ?? true,
    reload,
    setDetail,
    onDone: props.onDraftDone,
    now: now(),
    initialDiscuss: props.initialDiscuss,
  });
  useDrawerKeys(props.onClose, editing ? undefined : props.onStep);
  const column = columns.find((c) => c.key === item?.columnKey);
  const title = useRef<HTMLHeadingElement>(null);
  const loaded = item !== undefined;
  // Callers key the drawer by item, so ↑/↓ mounts it afresh and this runs again.
  useEffect(() => {
    if (loaded) {
      title.current?.focus();
    }
  }, [loaded]);

  const overview = {
    description:
      item === undefined ? null : (
        <ItemMarkdown
          api={api}
          text={item.description}
          attachments={attachments}
          onOpen={setViewing}
        />
      ),
    after: (
      <>
        {draft.after}
        <AttachmentsSection
          api={api}
          attachments={attachments}
          uploads={files.uploads}
          actions={editable ? files.actions : undefined}
          onOpen={setViewing}
        />
      </>
    ),
  };
  const done = (): void => {
    setEditing(false);
    reload();
  };

  return (
    <aside
      className={`item-drawer${draft.isDraft ? " item-drawer-draft" : ""}`}
      aria-label={draft.isDraft ? `Draft ${itemId}` : `Item ${itemId}`}
      {...(editable ? files.dropTarget : {})}
    >
      <DrawerHeader
        itemId={itemId}
        item={item}
        draft={draft.isDraft}
        onMove={draft.isDraft ? undefined : props.onMove}
        onEdit={editable && item !== undefined && !editing ? () => setEditing(true) : undefined}
        onClose={props.onClose}
        back={props.back}
      />
      {item === undefined ? (
        <p className="drawer-empty" role="status">
          {error === null ? "Loading…" : `Couldn't load ${itemId}: ${error}`}
        </p>
      ) : (
        <div className="drawer-body">
          <h2 className="drawer-title" tabIndex={-1} ref={title}>
            {item.title}
          </h2>
          {draft.isDraft ? null : (
            <>
              <Stepper column={column} />
              <MetaLine item={item} column={column} who={who} now={now()} />
              <NextStepView
                step={nextStep(columns, column, check)}
                assignee={item.assignee ? who(`bot:${item.assignee}`) : null}
              />
            </>
          )}
          {props.children}
          {draft.review}
          {editing ? (
            <ItemEditForm
              api={api}
              item={item}
              bots={bots}
              onPasteImage={files.pasteImage}
              onSaved={done}
              onCancel={done}
            />
          ) : detail ? (
            <DrawerTabs
              detail={detail}
              initialTab={props.initialTab}
              who={who}
              columnName={columnName}
              onComment={props.canComment ? comment : undefined}
              overview={overview}
            />
          ) : null}
        </div>
      )}
      {draft.isDraft ? draft.footer : props.footer}
      {draft.side}
      {draft.overlay}
      {viewing === null ? null : (
        <AttachmentViewer api={api} attachment={viewing} onClose={() => setViewing(null)} />
      )}
    </aside>
  );
}

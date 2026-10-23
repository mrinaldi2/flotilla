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

import { useCallback, useEffect, useState } from "react";
import type { ReactElement, ReactNode } from "react";
import { escapeIsTheDrawers } from "../../../hooks/useDrawerEscape";
import type { DaemonApi } from "../../../protocol/api";
import type { BoardApi } from "../../../protocol/board";
import type { Bot } from "../../../protocol/entities";
import type {
  BoardColumn,
  Item,
  ItemAttachment,
  ItemCard,
} from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { ColumnKind, ItemType } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { Question } from "../../../protocol/gen/flotilla/board/v1/questions_pb";
import type { ItemDetail } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { ARTIFACTS_V2 } from "../../../protocol/chat";
import { COLUMN_RESEARCH } from "../../../protocol/plan";
import { questionsOn } from "../../../protocol/questions";
import QuestionsSection from "../../questions/QuestionsSection";
import { useLiveQuestions } from "../../questions/useLiveQuestions";
import AttachmentsSection from "../attachments/AttachmentsSection";
import { documentsOf } from "../attachments/attachments";
import AttachmentViewer from "../attachments/AttachmentViewer";
import { useArtifactReader } from "../attachments/DocumentPreview";
import ItemMarkdown from "../attachments/ItemMarkdown";
import { useDrawerAttachments } from "../attachments/useDrawerAttachments";
import { useDraftDrawer } from "../drafts/useDraftDrawer";
import type { Anchor } from "../useBoardMoves";
import { AttachDesign, DesignSection } from "./DesignSection";
import CardPrs from "./CardPrs";
import DrawerBody from "./DrawerBody";
import { DrawerHeader } from "./DrawerParts";
import type { Tab } from "./DrawerTabsView";
import type { DrawerSize } from "./useDrawerSize";
import { DRAWER_MIN, useDrawerSize } from "./useDrawerSize";
import { useItemDetail } from "./useItemDetail";

export interface ItemDrawerProps {
  readonly api: BoardApi;
  readonly itemId: string;
  /** The card's version on the board: a push that moves it reads again. */
  readonly version?: bigint;
  /** A draft card's invited bots: a push that changes them reads again. */
  readonly invited?: readonly string[];
  readonly columns: readonly BoardColumn[];
  readonly bots: readonly Bot[];
  /** Whether this connection may comment (the control grant). */
  readonly canComment: boolean;
  /** Whether it may edit the item and its attachments; as `canComment` when unset. */
  readonly canEdit?: boolean;
  /** Whether it may answer the card's questions (the approve grant, 0.22.0). */
  readonly canAnswer?: boolean;
  readonly onClose: () => void;
  /** Opens "Move to…" below the button; absent where moves aren't offered. */
  readonly onMove?: (anchor: Anchor) => void;
  /** Steps to the previous (-1) or next (1) card in the column. */
  readonly onStep?: (direction: -1 | 1) => void;
  /** The clock, for "Doing for 31h"; tests and stories pin it. */
  readonly now?: () => number;
  /** The tab it opens on; Overview unless a story shows another. */
  readonly initialTab?: Tab;
  /** Its width; the owner's last one unless a story pins it (0.22.0). */
  readonly initialSize?: DrawerSize;
  /** Shown above the tabs: the item's commands for the owner (H-117). */
  readonly children?: ReactNode;
  /** The card shown before, for `‹ Back` (UX-035 §4). */
  readonly back?: { readonly id: string; readonly onBack: () => void };
  /** Below everything (a `drawer-foot`): "Open on the board" off the board. */
  readonly footer?: ReactNode;
  /** For a draft's docked chat ("Discuss with a bot"). */
  readonly client?: DaemonApi;
  /** The card's project, so its linked documents can be read (0.20.0). */
  readonly projectId?: string;
  readonly connected?: boolean;
  /** The project's lead, first in "Discuss with a bot". */
  readonly leadId?: string;
  /** A draft was finalized or discarded: what happened, in words. */
  readonly onDraftDone?: (message: string) => void;
  /** Stories: open in the edit form, or with a bot's chat docked. */
  readonly initialEdit?: boolean;
  readonly initialDiscuss?: string;
  /** Opens another card here: a task's design (0.21.0). */
  readonly onOpenItem?: (id: string) => void;
  readonly onOpenPr?: (number: number) => void; // its PRs' link (0.24.0)
  /** The board's open tasks, which a done Design card attaches to (0.21.0). */
  readonly designTasks?: readonly ItemCard[];
}

/** "bot:<id>", "user", "device:…", "daemon": who did something, in words. */
function whoNamer(bots: readonly Bot[]): (actor: string) => string {
  return (actor) => {
    if (actor === "user" || actor === "owner" || actor.startsWith("device:")) {
      return "You";
    }
    // The board's own move, on a fact such as the assignee starting work.
    if (actor === "daemon") {
      return "Automation";
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
      } else if (
        !typing &&
        onStep &&
        (event.key === "ArrowUp" || event.key === "ArrowDown") &&
        // A menu or dialog over the drawer has the arrows.
        !event.defaultPrevented &&
        document.querySelector('[role="menu"], [role="dialog"]') === null
      ) {
        event.preventDefault();
        onStep(event.key === "ArrowUp" ? -1 : 1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, onStep]);
}

/** Everything the drawer holds for one item. */
function useItemDrawer(props: ItemDrawerProps) {
  const { api, itemId, columns, bots } = props;
  const state = useItemDetail(api, itemId, props.version, props.invited?.join(" "));
  const { detail, setDetail, reload } = state;
  const [editing, setEditing] = useState(props.initialEdit === true);
  const [viewing, setViewing] = useState<ItemAttachment | null>(null);
  const editable = props.canEdit ?? props.canComment;
  const who = whoNamer(bots);
  const item = detail?.item;
  const attachments = detail?.attachments ?? [];
  const files = useDrawerAttachments(api, itemId, attachments, setDetail);
  const now = (props.now ?? Date.now)();
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
    now,
    initialDiscuss: props.initialDiscuss,
  });
  useDrawerKeys(props.onClose, editing ? undefined : props.onStep);
  return { state, editing, setEditing, viewing, setViewing, editable, who, files, now, draft };
}

/** A task's design cards, and on a done Design card the owner's "Attach design to a task…". */
function DesignParts(props: {
  readonly p: ItemDrawerProps;
  readonly item: Item;
  readonly detail: ItemDetail;
  readonly editable: boolean;
}): ReactElement {
  const { p, item } = props;
  const column = p.columns.find((c) => c.key === item.columnKey);
  const attachable =
    props.editable &&
    p.api.capabilities.includes(COLUMN_RESEARCH) &&
    item.type === ItemType.DESIGN &&
    column?.kind === ColumnKind.DONE;
  return (
    <>
      <DesignSection
        api={p.api}
        links={props.detail.links}
        columnName={(key) => p.columns.find((c) => c.key === key)?.name ?? key}
        onOpenItem={p.onOpenItem}
      />
      {attachable && p.designTasks !== undefined ? (
        <AttachDesign api={p.api} designId={item.id} tasks={p.designTasks} />
      ) : null}
    </>
  );
}

const NO_QUESTIONS: readonly Question[] = [];

/** The card's questions for the owner (0.22.0), kept live by the project's pushes. */
function CardQuestions(props: {
  readonly api: BoardApi;
  readonly itemId: string;
  readonly questions: readonly Question[];
  readonly canAnswer: boolean;
}): ReactElement | null {
  const { itemId } = props;
  const belongs = useCallback((q: Question): boolean => q.itemId === itemId, [itemId]);
  const live = useLiveQuestions(props.api, props.questions, belongs);
  return (
    <QuestionsSection
      api={props.api}
      questions={live.questions}
      canAnswer={props.canAnswer}
      onChanged={live.apply}
    />
  );
}

/** The item's attachments under its description, and a draft's "Discuss with a bot". */
function OverviewAfter(props: {
  readonly api: BoardApi;
  readonly design: ReactNode;
  readonly draftAfter: ReactNode;
  readonly attachments: readonly ItemAttachment[];
  readonly files: ReturnType<typeof useDrawerAttachments>;
  readonly editable: boolean;
  /** With `artifacts_v2`: its attachments and artifact links as one Documents list. */
  readonly documents: boolean;
  readonly onOpen: (attachment: ItemAttachment) => void;
}): ReactElement {
  return (
    <>
      {props.draftAfter}
      {props.design}
      <AttachmentsSection
        api={props.api}
        attachments={props.documents ? documentsOf(props.attachments) : props.attachments}
        title={props.documents ? "Documents" : undefined}
        uploads={props.files.uploads}
        actions={props.editable ? props.files.actions : undefined}
        onOpen={props.onOpen}
      />
    </>
  );
}

type DrawerState = ReturnType<typeof useItemDrawer>;

/** The item once read, or what stands in for it. */
function DrawerMain(props: { readonly p: ItemDrawerProps; readonly d: DrawerState }): ReactElement {
  const { p, d } = props;
  const { detail } = d.state;
  const item = detail?.item;
  if (item === undefined || detail === null) {
    return (
      <p className="drawer-empty" role="status">
        {d.state.error === null ? "Loading…" : `Couldn't load ${p.itemId}: ${d.state.error}`}
      </p>
    );
  }
  const documents = p.api.capabilities.includes(ARTIFACTS_V2);
  const overview = {
    description: (
      <ItemMarkdown
        api={p.api}
        text={item.description}
        attachments={detail.attachments}
        onOpen={d.setViewing}
      />
    ),
    after: (
      <OverviewAfter
        api={p.api}
        design={<DesignParts p={p} item={item} detail={detail} editable={d.editable} />}
        draftAfter={d.draft.after}
        attachments={detail.attachments}
        files={d.files}
        editable={d.editable}
        documents={documents}
        onOpen={d.setViewing}
      />
    ),
  };
  return (
    <DrawerBody
      api={p.api}
      detail={detail}
      item={item}
      check={d.state.check}
      columns={p.columns}
      bots={p.bots}
      who={d.who}
      now={d.now}
      isDraft={d.draft.isDraft}
      review={d.draft.review}
      overview={overview}
      editing={d.editing}
      onEdited={() => {
        d.setEditing(false);
        d.state.reload();
      }}
      onPasteImage={d.files.pasteImage}
      onComment={p.canComment ? d.state.comment : undefined}
      initialTab={p.initialTab}
      documents={documents}
      prs={
        <CardPrs
          client={p.client}
          projectId={p.projectId}
          connected={p.connected ?? true}
          itemId={p.itemId}
          now={d.now}
          onOpenPr={p.onOpenPr}
        />
      }
      questions={
        questionsOn(p.api) ? (
          <CardQuestions
            api={p.api}
            itemId={p.itemId}
            questions={detail.questions.length === 0 ? NO_QUESTIONS : detail.questions}
            canAnswer={p.canAnswer ?? p.canComment}
          />
        ) : null
      }
    >
      {p.children}
    </DrawerBody>
  );
}

export default function ItemDrawer(props: ItemDrawerProps): ReactElement {
  const { api, itemId } = props;
  const d = useItemDrawer(props);
  const readArtifact = useArtifactReader(props.client, props.projectId);
  const item = d.state.detail?.item;
  const isDraft = d.draft.isDraft;
  const canEdit = d.editable && item !== undefined && !d.editing;
  const sizing = useDrawerSize(props.initialSize);
  const classes = [
    "item-drawer",
    isDraft ? "item-drawer-draft" : "",
    sizing.wide ? "item-drawer-wide" : "",
  ];
  return (
    <aside
      className={classes.filter(Boolean).join(" ")}
      style={sizing.style}
      aria-label={`${isDraft ? "Draft" : "Item"} ${itemId}`}
      {...(d.editable ? d.files.dropTarget : {})}
    >
      {sizing.wide ? null : (
        <div
          className="drawer-edge"
          role="separator"
          aria-orientation="vertical"
          aria-label="Drawer width"
          aria-valuemin={DRAWER_MIN}
          tabIndex={0}
          title="Drag to resize · double-click to reset"
          {...sizing.edge}
        />
      )}
      <DrawerHeader
        itemId={itemId}
        item={item}
        draft={isDraft}
        onMove={isDraft ? undefined : props.onMove}
        onEdit={canEdit ? () => d.setEditing(true) : undefined}
        onClose={props.onClose}
        back={props.back}
        wide={{ on: sizing.wide, onToggle: sizing.onToggleWide }}
      />
      <DrawerMain p={props} d={d} />
      {isDraft ? d.draft.footer : props.footer}
      {d.draft.side}
      {d.draft.overlay}
      {d.viewing === null ? null : (
        <AttachmentViewer
          api={api}
          attachment={d.viewing}
          readArtifact={readArtifact ?? undefined}
          onClose={() => d.setViewing(null)}
        />
      )}
    </aside>
  );
}

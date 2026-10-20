// The item drawer's body (U4): the title, where the item stands (a live
// item's stepper, meta line and next step), the owner's commands, a draft's
// review, then the edit form or the tabs. Focus lands on the title once
// the item loads, so a screen reader reads it (UX-012).

import { useEffect, useRef } from "react";
import type { ReactElement, ReactNode } from "react";
import type { BoardApi } from "../../../protocol/board";
import type { Bot } from "../../../protocol/entities";
import type { BoardColumn, Item } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { ItemDetail, MoveCheck } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import ItemEditForm from "../edit/ItemEditForm";
import type { PostComment } from "./DrawerActivity";
import { MetaLine, NextStepView, Stepper } from "./DrawerParts";
import { DrawerTabs } from "./DrawerTabsView";
import type { Tab } from "./DrawerTabsView";
import { nextStep } from "./drawerText";

function LiveHead(props: {
  readonly item: Item;
  readonly columns: readonly BoardColumn[];
  readonly check: MoveCheck | null;
  readonly who: (actor: string) => string;
  readonly now: number;
}): ReactElement {
  const { item, columns, who } = props;
  const column = columns.find((c) => c.key === item.columnKey);
  return (
    <>
      <Stepper columns={columns} column={column} />
      <MetaLine item={item} column={column} who={who} now={props.now} />
      <NextStepView
        step={nextStep(columns, column, props.check)}
        assignee={item.assignee ? who(`bot:${item.assignee}`) : null}
      />
    </>
  );
}

export interface DrawerBodyProps {
  readonly api: BoardApi;
  readonly detail: ItemDetail;
  readonly item: Item;
  readonly check: MoveCheck | null;
  readonly columns: readonly BoardColumn[];
  readonly bots: readonly Bot[];
  readonly who: (actor: string) => string;
  readonly now: number;
  readonly isDraft: boolean;
  /** A draft's Review list. */
  readonly review: ReactNode;
  /** Overview's description, drawn with its images, and what follows it. */
  readonly overview: { readonly description: ReactNode; readonly after: ReactNode };
  readonly editing: boolean;
  readonly onEdited: () => void;
  readonly onPasteImage: (file: File) => Promise<string | null>;
  readonly onComment?: PostComment;
  readonly initialTab?: Tab;
  /** The card's questions for the owner, above the tabs and their comments (0.22.0). */
  readonly questions?: ReactNode;
  /** Artifact links listed with the documents, so the Links tab leaves them out. */
  readonly documents?: boolean;
  /** The item's commands for the owner (H-117). */
  readonly children?: ReactNode;
}

export default function DrawerBody(props: DrawerBodyProps): ReactElement {
  const { item, columns } = props;
  const title = useRef<HTMLHeadingElement>(null);
  // Callers key the drawer by item, so ↑/↓ mounts it afresh and this runs again.
  useEffect(() => {
    title.current?.focus();
  }, []);
  const columnName = (key: string): string => columns.find((c) => c.key === key)?.name ?? key;
  return (
    <div className="drawer-body">
      <h2 className="drawer-title" tabIndex={-1} ref={title}>
        {item.title}
      </h2>
      {props.isDraft ? null : (
        <LiveHead
          item={item}
          columns={columns}
          check={props.check}
          who={props.who}
          now={props.now}
        />
      )}
      {props.children}
      {props.review}
      {props.editing ? null : props.questions}
      {props.editing ? (
        <ItemEditForm
          api={props.api}
          item={item}
          bots={props.bots}
          onPasteImage={props.onPasteImage}
          onSaved={props.onEdited}
          onCancel={props.onEdited}
        />
      ) : (
        <DrawerTabs
          detail={props.detail}
          initialTab={props.initialTab}
          who={props.who}
          columnName={columnName}
          onComment={props.onComment}
          overview={props.overview}
          documents={props.documents}
        />
      )}
    </div>
  );
}

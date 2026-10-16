// The item drawer's tabs (U4): Overview, Links and Activity, the counts in
// the tab names.

import { useState } from "react";
import type { ReactElement, ReactNode } from "react";
import { ItemEventKind } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { ItemDetail } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { Activity } from "./DrawerActivity";
import type { PostComment } from "./DrawerActivity";
import { Links, Overview } from "./DrawerTabs";

export type Tab = "overview" | "links" | "activity";

export function DrawerTabs(props: {
  readonly detail: ItemDetail;
  readonly initialTab: Tab | undefined;
  readonly who: (actor: string) => string;
  readonly columnName: (key: string) => string;
  readonly onComment?: PostComment;
  /** Overview's description, drawn with its images, and what follows it. */
  readonly overview?: { readonly description: ReactNode; readonly after: ReactNode };
}): ReactElement {
  const { detail, who } = props;
  const [tab, setTab] = useState<Tab>(props.initialTab ?? "overview");
  // What Activity lists: each comment once, not again as its history event.
  const moves = detail.history.filter((e) => e.kind !== ItemEventKind.COMMENTED).length;
  const tabs: readonly (readonly [Tab, string])[] = [
    ["overview", "Overview"],
    ["links", `Links ${detail.links.length}`],
    ["activity", `Activity ${moves + detail.comments.length}`],
  ];
  return (
    <>
      <div className="drawer-tabs" role="tablist" aria-label="Item">
        {tabs.map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={tab === id}
            className={tab === id ? "on" : ""}
            onClick={() => setTab(id)}
          >
            {label}
          </button>
        ))}
      </div>
      <div role="tabpanel">
        {tab === "overview" ? (
          <Overview
            detail={detail}
            who={who}
            description={props.overview?.description}
            after={props.overview?.after}
          />
        ) : null}
        {tab === "links" ? <Links links={detail.links} /> : null}
        {tab === "activity" ? (
          <Activity
            detail={detail}
            who={who}
            columnName={props.columnName}
            onComment={props.onComment}
          />
        ) : null}
      </div>
    </>
  );
}

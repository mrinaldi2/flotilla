// What a draft adds to the item drawer (0.19.0), as slots the drawer
// places: the Review list, "Discuss with a bot" under the description, the
// footer with the changes strip, Discard and Finalize, the bot's chat
// docked beside it, and the Discard dialog.

import { useState } from "react";
import type { ReactNode } from "react";
import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { DaemonApi } from "../../../protocol/api";
import type { Bot } from "../../../protocol/entities";
import type { BoardColumn, Item, Unmet } from "../../../protocol/gen/hermes/board/v1/board_pb";
import type { ItemDetail } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { errText } from "../../../util";
import DiscussButton from "./DiscussButton";
import DiscussPanel from "./DiscussPanel";
import { ChangesStrip, DiscardDialog, DraftFooter, DraftReview } from "./DraftParts";
import { useDraft } from "./useDraft";

export interface DraftDrawerParams {
  readonly api: BoardApi;
  /** For the docked chat; without it, no "Discuss with a bot". */
  readonly client?: DaemonApi;
  readonly itemId: string;
  readonly item: Item | undefined;
  readonly detail: ItemDetail | null;
  readonly bots: readonly Bot[];
  readonly leadId?: string;
  /** The draft's board's columns. */
  readonly columns: readonly BoardColumn[];
  readonly who: (actor: string) => string;
  readonly connected: boolean;
  readonly reload: () => void;
  readonly setDetail: (detail: ItemDetail) => void;
  /** The draft left the tray: finalized or discarded, in words. */
  readonly onDone?: (message: string) => void;
  readonly now: number;
  /** Opens the docked chat with this bot at once (stories). */
  readonly initialDiscuss?: string;
}

export interface DraftSlots {
  readonly isDraft: boolean;
  readonly review: ReactNode;
  readonly after: ReactNode;
  readonly footer: ReactNode;
  readonly side: ReactNode;
  readonly overlay: ReactNode;
}

export function useDraftDrawer(p: DraftDrawerParams): DraftSlots {
  const { api, item } = p;
  const isDraft = item?.state === "draft";
  const draft = useDraft(api, p.itemId, item?.version, p.reload, isDraft);
  const [discussWith, setDiscussWith] = useState<string | null>(p.initialDiscuss ?? null);
  const [discarding, setDiscarding] = useState(false);
  const [busy, setBusy] = useState(false);
  const [unmet, setUnmet] = useState<readonly Unmet[] | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  if (!isDraft || item === undefined) {
    return { isDraft: false, review: null, after: null, footer: null, side: null, overlay: null };
  }
  const version = item.version;
  const bot = p.bots.find((b) => b.id === discussWith);

  const discuss = (chosen: Bot): void => {
    setFailure(null);
    void (async () => {
      try {
        const invite = { id: item.id, bot: chosen.id };
        p.setDetail(await boardCall(api, { case: "itemInvite", value: invite }, "item"));
        setDiscussWith(chosen.id);
      } catch (error) {
        setFailure(errText(error));
      }
    })();
  };
  const finalize = (column: string): void => {
    setBusy(true);
    setUnmet(null);
    setFailure(null);
    void (async () => {
      const outcome = await draft.finalize(column, version);
      setBusy(false);
      if (outcome.kind === "done") {
        const name = p.columns.find((c) => c.key === outcome.column)?.name ?? outcome.column;
        p.reload();
        p.onDone?.(`${item.id} is on the board, in ${name}.`);
      } else if (outcome.kind === "refused") {
        setUnmet(outcome.unmet);
      } else if (outcome.kind === "conflict") {
        setFailure(`${item.id} changed meanwhile; look at it again, then finalize.`);
      } else {
        setFailure(outcome.message);
      }
    })();
  };

  const error = failure ?? draft.error;
  return {
    isDraft: true,
    review:
      draft.reviewing === null ? null : (
        <DraftReview
          changes={draft.reviewing}
          undone={draft.undone}
          who={p.who}
          onUndo={(revision) => void draft.undo(revision, version)}
          onClose={draft.closeReview}
        />
      ),
    after:
      p.client === undefined ? null : (
        <DiscussButton
          bots={p.bots}
          leadId={p.leadId}
          invited={p.detail?.invited ?? []}
          onDiscuss={discuss}
        />
      ),
    footer: (
      <>
        <ChangesStrip unseen={draft.unseen} who={p.who} onReview={draft.review} />
        {error === null ? null : (
          <p className="field-error draft-error" role="alert">
            {error}
          </p>
        )}
        <DraftFooter
          columns={p.columns}
          busy={busy}
          unmet={unmet}
          onFinalize={finalize}
          onDiscard={() => setDiscarding(true)}
        />
      </>
    ),
    side:
      p.client === undefined || bot === undefined ? null : (
        <DiscussPanel
          client={p.client}
          bot={bot}
          itemId={item.id}
          connected={p.connected}
          changes={draft.unseen.length}
          onReview={draft.review}
          onClose={() => setDiscussWith(null)}
          now={p.now}
        />
      ),
    overlay: discarding ? (
      <DiscardDialog
        itemId={item.id}
        onCancel={() => setDiscarding(false)}
        onDiscard={(reason) => {
          setDiscarding(false);
          void (async () => {
            if (await draft.discard(reason, version)) {
              p.reload();
              p.onDone?.(`${item.id} was discarded.`);
            }
          })();
        }}
      />
    ) : null,
  };
}

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
import type { DraftState, FinalizeOutcome } from "./useDraft";

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

const NONE: DraftSlots = {
  isDraft: false,
  review: null,
  after: null,
  footer: null,
  side: null,
  overlay: null,
};

/** What the drawer says after Finalize, beyond a refusal. */
function finalized(
  outcome: FinalizeOutcome,
  itemId: string,
  columns: readonly BoardColumn[],
): { readonly done?: string; readonly failure?: string } {
  switch (outcome.kind) {
    case "done":
      return {
        done: `${itemId} is on the board, in ${columns.find((c) => c.key === outcome.column)?.name ?? outcome.column}.`,
      };
    case "conflict":
      return { failure: `${itemId} changed meanwhile; look at it again, then finalize.` };
    case "failed":
      return { failure: outcome.message };
    default:
      return {};
  }
}

/** Finalize, Discard and Discuss: their busy state and their refusals. */
function useDraftActions(p: DraftDrawerParams, draft: DraftState) {
  const [discussWith, setDiscussWith] = useState<string | null>(p.initialDiscuss ?? null);
  const [discarding, setDiscarding] = useState(false);
  const [busy, setBusy] = useState(false);
  const [unmet, setUnmet] = useState<readonly Unmet[] | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const id = p.itemId;
  const version = p.item?.version ?? 0n;
  const left = (message: string): void => {
    p.reload();
    p.onDone?.(message);
  };
  return {
    discussWith,
    closeDiscuss: () => setDiscussWith(null),
    discarding,
    setDiscarding,
    busy,
    unmet,
    error: failure ?? draft.error,
    discuss: (chosen: Bot): void => {
      setFailure(null);
      void (async () => {
        try {
          const invite = { id, bot: chosen.id };
          p.setDetail(await boardCall(p.api, { case: "itemInvite", value: invite }, "item"));
          setDiscussWith(chosen.id);
        } catch (error) {
          setFailure(errText(error));
        }
      })();
    },
    finalize: (column: string): void => {
      setBusy(true);
      setUnmet(null);
      setFailure(null);
      void (async () => {
        const outcome = await draft.finalize(column, version);
        setBusy(false);
        setUnmet(outcome.kind === "refused" ? outcome.unmet : null);
        const said = finalized(outcome, id, p.columns);
        setFailure(said.failure ?? null);
        if (said.done !== undefined) {
          left(said.done);
        }
      })();
    },
    discard: (reason: string): void => {
      setDiscarding(false);
      void (async () => {
        if (await draft.discard(reason, version)) {
          left(`${id} was discarded.`);
        }
      })();
    },
  };
}

export function useDraftDrawer(p: DraftDrawerParams): DraftSlots {
  const { api, item } = p;
  const isDraft = item?.state === "draft";
  const draft = useDraft(api, p.itemId, item?.version, p.reload, isDraft);
  const actions = useDraftActions(p, draft);
  if (!isDraft || item === undefined) {
    return NONE;
  }
  const bot = p.bots.find((b) => b.id === actions.discussWith);
  return {
    isDraft: true,
    review:
      draft.reviewing === null ? null : (
        <DraftReview
          changes={draft.reviewing}
          undone={draft.undone}
          who={p.who}
          onUndo={(revision) => void draft.undo(revision, item.version)}
          onClose={draft.closeReview}
        />
      ),
    after:
      p.client === undefined ? null : (
        <DiscussButton
          bots={p.bots}
          leadId={p.leadId}
          invited={p.detail?.invited ?? []}
          onDiscuss={actions.discuss}
        />
      ),
    footer: (
      <>
        <ChangesStrip unseen={draft.unseen} who={p.who} onReview={draft.review} />
        {actions.error === null ? null : (
          <p className="field-error draft-error" role="alert">
            {actions.error}
          </p>
        )}
        <DraftFooter
          columns={p.columns}
          busy={actions.busy}
          unmet={actions.unmet}
          onFinalize={actions.finalize}
          onDiscard={() => actions.setDiscarding(true)}
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
          onClose={actions.closeDiscuss}
          now={p.now}
        />
      ),
    overlay: actions.discarding ? (
      <DiscardDialog
        itemId={item.id}
        onCancel={() => actions.setDiscarding(false)}
        onDiscard={actions.discard}
      />
    ) : null,
  };
}

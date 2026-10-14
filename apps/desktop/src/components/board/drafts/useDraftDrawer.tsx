// What a draft adds to the item drawer (0.19.0), as slots the drawer
// places: the Review list, "Discuss with a bot" and the bots discussing it
// under the description, the footer with the changes strip, Discard and
// Finalize, the bot's chat docked beside it, and the Discard and Remove
// dialogs. A bot no longer invited, here or from another device, has its
// docked chat closed. While an invite or a removal is on its way, its
// control is disabled and further clicks are ignored, so a double click
// sends one request.

import { useRef, useState } from "react";
import type { ReactNode } from "react";
import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { DaemonApi } from "../../../protocol/api";
import type { Bot } from "../../../protocol/entities";
import type { BoardColumn, Item, Unmet } from "../../../protocol/gen/hermes/board/v1/board_pb";
import type { ItemDetail } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { errText } from "../../../util";
import DiscussButton from "./DiscussButton";
import { DiscussedWith, invitees, RemoveDialog } from "./DiscussedWith";
import type { Invitee } from "./DiscussedWith";
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

/**
 * One request at a time per key: `run` ignores a key already on its way.
 * The ref answers at once, before React re-renders the disabled control;
 * the state is what the control shows.
 */
function useInFlight(): {
  readonly pending: readonly string[];
  readonly run: (key: string, work: () => Promise<void>) => void;
} {
  const flying = useRef(new Set<string>());
  const [pending, setPending] = useState<readonly string[]>([]);
  const run = (key: string, work: () => Promise<void>): void => {
    if (flying.current.has(key)) {
      return;
    }
    flying.current.add(key);
    setPending([...flying.current]);
    void (async () => {
      try {
        await work();
      } finally {
        flying.current.delete(key);
        setPending([...flying.current]);
      }
    })();
  };
  return { pending, run };
}

/** Finalize, Discard and Discuss: their busy state and their refusals. */
function useDraftActions(p: DraftDrawerParams, draft: DraftState) {
  const [discussWith, setDiscussWith] = useState<string | null>(p.initialDiscuss ?? null);
  const [asking, setAsking] = useState<Invitee | null>(null);
  const inviting = useInFlight();
  const uninviting = useInFlight();
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
    removing: asking,
    /** Asks before removing; not again for a bot whose removal is on its way. */
    setRemoving: (invitee: Invitee | null): void => {
      if (invitee === null || !uninviting.pending.includes(invitee.id)) {
        setAsking(invitee);
      }
    },
    uninviting: uninviting.pending,
    remove: (invitee: Invitee): void => {
      setAsking(null);
      uninviting.run(invitee.id, async () => {
        setFailure(null);
        try {
          const uninvite = { id, bot: invitee.id, remove: true };
          p.setDetail(await boardCall(p.api, { case: "itemInvite", value: uninvite }, "item"));
          setDiscussWith((open) => (open === invitee.id ? null : open));
        } catch (error) {
          setFailure(errText(error));
        }
      });
    },
    discarding,
    setDiscarding,
    busy,
    unmet,
    error: failure ?? draft.error,
    inviting: inviting.pending.length > 0,
    discuss: (chosen: Bot): void => {
      // One invite at a time, whichever bot: the menu is one control.
      inviting.run("invite", async () => {
        setFailure(null);
        try {
          const invite = { id, bot: chosen.id };
          p.setDetail(await boardCall(p.api, { case: "itemInvite", value: invite }, "item"));
          setDiscussWith(chosen.id);
        } catch (error) {
          setFailure(errText(error));
        }
      });
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
  const invited = p.detail?.invited ?? [];
  // Once the item is read, a bot it no longer invites has no docked chat.
  const docked = p.detail === null || invited.includes(actions.discussWith ?? "");
  const bot = docked ? p.bots.find((b) => b.id === actions.discussWith) : undefined;
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
        <>
          <DiscussButton
            bots={p.bots}
            leadId={p.leadId}
            invited={invited}
            busy={actions.inviting}
            onDiscuss={actions.discuss}
          />
          <DiscussedWith
            invitees={invitees(invited, p.bots)}
            removing={actions.uninviting}
            onRemove={actions.setRemoving}
          />
        </>
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
    overlay: overlay(item.id, actions),
  };
}

/** The dialog over the drawer, if one is open: Discard's or Remove's. */
function overlay(itemId: string, actions: ReturnType<typeof useDraftActions>): ReactNode {
  if (actions.discarding) {
    return (
      <DiscardDialog
        itemId={itemId}
        onCancel={() => actions.setDiscarding(false)}
        onDiscard={actions.discard}
      />
    );
  }
  const { removing } = actions;
  if (removing === null) {
    return null;
  }
  return (
    <RemoveDialog
      itemId={itemId}
      invitee={removing}
      onCancel={() => actions.setRemoving(null)}
      onRemove={() => actions.remove(removing)}
    />
  );
}

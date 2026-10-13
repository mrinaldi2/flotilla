// A draft in the owner's drawer (0.19.0): its changes since the owner last
// looked, read again whenever the draft's version moves; Review marks them
// seen (which clears its Needs you row) and keeps them listed, each with
// its own Undo; Finalize puts it on the board in a column, whose entry
// rules can refuse it; Discard drops it with a reason.

import { useCallback, useEffect, useState } from "react";
import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { DraftRevision, Unmet } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { errText } from "../../../util";
import { unseenChanges } from "./draftChanges";

export type FinalizeOutcome =
  | { readonly kind: "done"; readonly column: string }
  | { readonly kind: "refused"; readonly unmet: readonly Unmet[] }
  | { readonly kind: "conflict" }
  | { readonly kind: "failed"; readonly message: string };

export interface DraftState {
  /** The bots' changes the owner hasn't looked at. */
  readonly unseen: readonly DraftRevision[];
  /** What Review lists: the changes as they were when the owner opened it. */
  readonly reviewing: readonly DraftRevision[] | null;
  /** Revisions taken back in this drawer. */
  readonly undone: ReadonlySet<bigint>;
  readonly error: string | null;
  readonly review: () => void;
  readonly closeReview: () => void;
  readonly undo: (revision: DraftRevision, version: bigint) => Promise<void>;
  readonly finalize: (column: string, version: bigint) => Promise<FinalizeOutcome>;
  readonly discard: (reason: string, version: bigint) => Promise<boolean>;
}

export function useDraft(
  api: BoardApi,
  itemId: string,
  /** The draft's version: a bot's change reads the changes again. */
  version: bigint | undefined,
  /** Reads the item again after an undo. */
  reload: () => void,
  /** Only a draft has changes to follow. */
  active: boolean,
): DraftState {
  const [unseen, setUnseen] = useState<readonly DraftRevision[]>([]);
  const [reviewing, setReviewing] = useState<readonly DraftRevision[] | null>(null);
  const [undone, setUndone] = useState<ReadonlySet<bigint>>(new Set());
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!active) {
      return undefined;
    }
    let live = true;
    boardCall(api, { case: "itemDraftChanges", value: { id: itemId } }, "draftChanges").then(
      (changes) => live && setUnseen(unseenChanges(changes.revisions, changes.seen)),
      (failure: unknown) => live && setError(errText(failure)),
    );
    return () => {
      live = false;
    };
    // `version` is a trigger: a bot's change reads the changes again.
    // oxlint-disable-next-line react/exhaustive-effect-dependencies
  }, [api, itemId, version, active]);

  const review = useCallback((): void => {
    const shown = unseen;
    setReviewing(shown);
    setUnseen([]);
    const last = shown.at(-1);
    if (last !== undefined) {
      boardCall(
        api,
        { case: "itemDraftChanges", value: { id: itemId, since: last.id - 1n, markSeen: true } },
        "draftChanges",
      ).then(
        (changes) => {
          // Marking seen covers every change up to the newest, including one
          // a bot made after the drawer read them: list it here, or the
          // owner never sees it.
          const later = unseenChanges(changes.revisions, last.id);
          if (later.length > 0) {
            setReviewing((listed) => [...(listed ?? []), ...later]);
          }
          return undefined;
        },
        (failure: unknown) => setError(errText(failure)),
      );
    }
  }, [api, itemId, unseen]);

  const undo = useCallback(
    async (revision: DraftRevision, at: bigint): Promise<void> => {
      setError(null);
      try {
        const result = await boardCall(
          api,
          {
            case: "itemDraftUndo",
            value: { id: itemId, expectedVersion: at, revision: revision.id },
          },
          "edited",
        );
        if (result.outcome.case === "done") {
          setUndone((all) => new Set([...all, revision.id]));
        } else if (result.outcome.case === "refused") {
          setError(result.outcome.value.unmet.map((u) => u.text).join(" "));
        } else {
          setError("The draft changed meanwhile; look again and undo once more.");
        }
      } catch (failure) {
        setError(errText(failure));
      }
      reload();
    },
    [api, itemId, reload],
  );

  const finalize = useCallback(
    async (column: string, at: bigint): Promise<FinalizeOutcome> => {
      try {
        const result = await boardCall(
          api,
          { case: "itemFinalize", value: { id: itemId, expectedVersion: at, column } },
          "moved",
        );
        switch (result.outcome.case) {
          case "done":
            return { kind: "done", column: result.outcome.value.columnKey };
          case "refused":
            return { kind: "refused", unmet: result.outcome.value.unmet };
          default:
            reload();
            return { kind: "conflict" };
        }
      } catch (failure) {
        return { kind: "failed", message: errText(failure) };
      }
    },
    [api, itemId, reload],
  );

  const discard = useCallback(
    async (reason: string, at: bigint): Promise<boolean> => {
      try {
        const result = await boardCall(
          api,
          { case: "itemDiscard", value: { id: itemId, expectedVersion: at, reason } },
          "edited",
        );
        if (result.outcome.case === "done") {
          return true;
        }
        setError(
          result.outcome.case === "refused"
            ? result.outcome.value.unmet.map((u) => u.text).join(" ")
            : "The draft changed meanwhile; look at it again.",
        );
      } catch (failure) {
        setError(errText(failure));
      }
      return false;
    },
    [api, itemId],
  );

  return {
    unseen,
    reviewing,
    undone,
    error,
    review,
    closeReview: () => setReviewing(null),
    undo,
    finalize,
    discard,
  };
}

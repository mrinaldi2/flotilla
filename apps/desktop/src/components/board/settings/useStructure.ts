// The owner's structure requests (0.19.0): boards, columns, rules and
// automations, and the ruling on a lead's proposal. Each is checked against
// the board version read; the snapshot an applied change answers with is
// adopted at once, so the next edit carries the new version.

import { useCallback, useState } from "react";
import type { BoardApi, BoardCall } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type {
  BoardSnapshot,
  StructureResult,
} from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { errText } from "../../../util";

/** The structure arms of a board request. */
type StructureCall = Extract<
  BoardCall,
  {
    readonly case:
      | "boardCreate"
      | "boardUpdate"
      | "columnAdd"
      | "columnUpdate"
      | "columnRemove"
      | "automationSet"
      | "boardProposalRule";
  }
>;

export interface Structure {
  /** Sends `call`; resolves with the result, or null when it was refused (see `error`). */
  readonly run: (call: StructureCall) => Promise<StructureResult | null>;
  readonly busy: boolean;
  /** Flotilla service's refusal of the last call, in its words. */
  readonly error: string | null;
  readonly clearError: () => void;
}

export function useStructure(
  api: BoardApi,
  /** Takes the board as an applied change left it. */
  adopt: (snapshot: BoardSnapshot) => void,
): Structure {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(
    async (call: StructureCall): Promise<StructureResult | null> => {
      setBusy(true);
      setError(null);
      try {
        const result = await boardCall(api, call, "structure");
        if (result.outcome.case === "applied") {
          adopt(result.outcome.value);
        }
        return result;
      } catch (failure) {
        setError(errText(failure));
        return null;
      } finally {
        setBusy(false);
      }
    },
    [api, adopt],
  );
  const clearError = useCallback(() => setError(null), []);
  return { run, busy, error, clearError };
}

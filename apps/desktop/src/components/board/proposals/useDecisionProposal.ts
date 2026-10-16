// A lead's structure proposal raises one decision (0.19.0), and approving
// or declining the proposal is how it is ruled, so Decisions shows the
// proposal for it. A decision doesn't name its proposal: the project's
// board snapshot carries the open proposals, each with its decision. The
// board is read only for a decision worded as a proposal's (`mayBeProposal`).

import { useCallback, useEffect, useState } from "react";
import type { DaemonApi } from "../../../protocol/api";
import { boardCall, PROTO_ENCODING } from "../../../protocol/board";
import type { Decision } from "../../../protocol/decisions";
import type { Board, BoardColumn } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { BoardProposal } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { errText } from "../../../util";

export interface FoundProposal {
  readonly proposal: BoardProposal;
  /** The board it changes; unset for a new board. */
  readonly board: Board | undefined;
  /** That board's columns as they are now. */
  readonly columns: readonly BoardColumn[];
}

export interface DecisionProposal {
  readonly found: FoundProposal | undefined;
  readonly busy: boolean;
  readonly error: string | null;
  /** Approves (applies) or declines it; the decision settles either way. */
  readonly rule: (approve: boolean, note?: string) => Promise<void>;
}

/**
 * Whether a decision reads as one the Flotilla service raised for a lead's
 * proposal: a question whose body ends "Proposal <id>.". Only then is the
 * project's board read, since `board_get` starts a board where there is
 * none, and Decisions must not do that for every project it shows.
 */
function mayBeProposal(decision: Decision): boolean {
  return decision.kind === "question" && /\bProposal [\w-]+\.$/.test(decision.body.trim());
}

export function useDecisionProposal(
  client: DaemonApi,
  connected: boolean,
  decision: Decision | undefined,
): DecisionProposal {
  const [found, setFound] = useState<FoundProposal | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const decisionId =
    decision?.state === "open" && mayBeProposal(decision) ? decision.id : undefined;
  const projectId = decision?.project_id;
  const served = client.encodings.includes(PROTO_ENCODING);
  useEffect(() => {
    if (!connected || !served || decisionId === undefined || projectId === undefined) {
      return undefined;
    }
    let live = true;
    const load = async (): Promise<void> => {
      let next: FoundProposal | undefined;
      try {
        const s = await boardCall(client, { case: "boardGet", value: { projectId } }, "board");
        const proposal = s.proposals.find((p) => p.decisionId === decisionId && p.state === "open");
        if (proposal !== undefined) {
          next = {
            proposal,
            board: s.boards.find((b) => b.id === proposal.boardId),
            columns: s.columns
              .filter((c) => c.boardId === proposal.boardId)
              // oxlint-disable-next-line unicorn/no-array-sort
              .sort((a, b) => a.ord - b.ord),
          };
        }
      } catch {
        // No board here, or none of its proposals: show the decision.
      }
      if (live) {
        setFound(next);
      }
    };
    void load();
    return () => {
      live = false;
    };
  }, [client, connected, served, decisionId, projectId]);

  const rule = useCallback(
    async (approve: boolean, note?: string): Promise<void> => {
      if (found === undefined) {
        return;
      }
      setBusy(true);
      setError(null);
      try {
        await boardCall(
          client,
          { case: "boardProposalRule", value: { proposalId: found.proposal.id, approve, note } },
          "structure",
        );
        setFound(undefined);
      } catch (failure) {
        setError(errText(failure));
      } finally {
        setBusy(false);
      }
    },
    [client, found],
  );

  return {
    found: found !== undefined && found.proposal.decisionId === decisionId ? found : undefined,
    busy,
    error,
    rule,
  };
}

// The card's pull requests at the top of its drawer (0.24.0): each one's
// state, live, and a link to it, so the owner goes from the card to its PR.

import type { ReactElement } from "react";
import type { DaemonApi } from "../../../protocol/api";
import { PULL_REQUESTS } from "../../../protocol/prs";
import { usePrList } from "../../prs/usePullRequests";
import { cardPrState } from "./cardPrText";

interface CardPrsProps {
  readonly client?: DaemonApi;
  readonly projectId?: string;
  readonly connected: boolean;
  readonly itemId: string;
  readonly now: number;
  /** Opens the PR in the project's Pull requests tab. */
  readonly onOpenPr?: (number: number) => void;
}

export default function CardPrs(props: CardPrsProps): ReactElement | null {
  const { client, projectId } = props;
  if (client === undefined || projectId === undefined) {
    return null;
  }
  if (!client.capabilities.includes(PULL_REQUESTS)) {
    return null;
  }
  return <Listed {...props} client={client} projectId={projectId} />;
}

function Listed(
  props: CardPrsProps & { readonly client: DaemonApi; readonly projectId: string },
): ReactElement | null {
  const list = usePrList(props.client, props.projectId, props.connected);
  const prs = (list.data ?? []).filter((pr) => pr.itemId === props.itemId);
  if (prs.length === 0) {
    return null;
  }
  return (
    <section className="drawer-prs" aria-label="Pull requests">
      {prs.map((pr) => {
        const state = cardPrState(pr, props.now);
        return (
          <div key={`${pr.repo}-${pr.number}`} className="drawer-pr">
            {props.onOpenPr ? (
              <button
                type="button"
                className="cc-link mono"
                aria-label={`Open pull request #${pr.number}`}
                onClick={() => props.onOpenPr?.(pr.number)}
              >
                PR #{pr.number}
              </button>
            ) : (
              <span className="mono">PR #{pr.number}</span>
            )}
            <span className="drawer-pr-title">{pr.title}</span>
            <span className={`pr-tone-${state.tone}`}>{state.text}</span>
          </div>
        );
      })}
    </section>
  );
}

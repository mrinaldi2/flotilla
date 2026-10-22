// A push request (0.24.0, `push_requests_v1`): a bot asks the owner to push
// its branch. The exact command, what it pushes, why, and Push in one click
// with the hash the card shows; See the diff on GitHub, or Reject with a
// reason. Without the approve grant the card says where it can be pushed.

import { ArrowUp } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";
import { openExternalUrl } from "../../externalUrl";
import type { OwnerAction } from "../../protocol/ownerActions";
import CardLink from "../cards/CardLink";
import { useNow } from "../control/useNow";
import ActionOutput from "./ActionOutput";
import type { PushDetails } from "./ownerActionText";
import { commitLine, diffUrl, expiresIn, pushResultLine } from "./ownerActionText";
import RejectForm from "./RejectForm";

interface PushCardProps {
  readonly action: OwnerAction;
  readonly push: PushDetails;
  readonly output?: string;
  readonly proposer: string;
  readonly canRun: boolean;
  readonly onRun: (action: OwnerAction) => void;
  readonly onReject: (action: OwnerAction, reason: string) => void;
  readonly now?: number;
}

/** Branch, Commit, Repo, Card and Why, as a grid. */
function Details(props: { readonly push: PushDetails; readonly reason: string }): ReactElement {
  const { push } = props;
  return (
    <dl className="push-card-details">
      <dt>Branch</dt>
      <dd className="mono">
        {push.target === push.branch ? push.branch : `${push.branch} → ${push.target}`}
      </dd>
      <dt>Commit</dt>
      <dd className="mono">{commitLine(push)}</dd>
      <dt>Repo</dt>
      <dd>{push.repo}</dd>
      {push.item_id === undefined || push.item_id.length === 0 ? null : (
        <>
          <dt>Card</dt>
          <dd>
            <CardLink id={push.item_id} />
          </dd>
        </>
      )}
      <dt>Why</dt>
      <dd>{`“${props.reason}”`}</dd>
    </dl>
  );
}

/** Push, See the diff and Reject…; Reject… opens its reason. */
function Buttons(
  props: Pick<PushCardProps, "action" | "push" | "onRun" | "onReject">,
): ReactElement {
  const [rejecting, setRejecting] = useState(false);
  const { action } = props;
  const diff = diffUrl(props.push);
  if (rejecting) {
    return (
      <RejectForm
        onCancel={() => setRejecting(false)}
        onReject={(reason) => props.onReject(action, reason)}
      />
    );
  }
  return (
    <div className="push-card-buttons">
      <button type="button" className="btn btn-primary" onClick={() => props.onRun(action)}>
        Push
      </button>
      {diff === null ? null : (
        <button
          type="button"
          className="btn"
          onClick={() => {
            openExternalUrl(diff).catch(() => undefined);
          }}
        >
          See the diff
        </button>
      )}
      <button type="button" className="btn push-card-reject" onClick={() => setRejecting(true)}>
        Reject…
      </button>
    </div>
  );
}

export default function PushCard(props: PushCardProps): ReactElement {
  const { action, push, proposer } = props;
  const clock = useNow();
  const waiting = action.state === "proposed";
  const expires = waiting ? expiresIn(action, props.now ?? clock) : null;
  const result = pushResultLine(action, proposer);
  const title = `${proposer} asks you to push`;
  return (
    <article className="owner-action push-card" aria-label={`${title}: ${push.branch}`}>
      <div className="push-card-head">
        <ArrowUp size={16} aria-hidden="true" className="push-card-icon" />
        <span className="owner-action-title">{title}</span>
        {expires === null ? null : <span className="push-card-expires">{expires}</span>}
      </div>
      <pre className="owner-action-content">{action.content}</pre>
      <Details push={push} reason={action.reason} />
      {result === null ? null : (
        <p className={`owner-action-result owner-action-${action.state}`} role="status">
          {result}
        </p>
      )}
      <ActionOutput action={action} live={props.output} />
      {waiting && props.canRun ? (
        <Buttons action={action} push={push} onRun={props.onRun} onReject={props.onReject} />
      ) : null}
      {waiting && !props.canRun ? (
        <p className="owner-action-hint">You can push this from a device with approve access.</p>
      ) : null}
      {waiting ? (
        <p className="push-card-note">
          Runs exactly this command as you, from the bot&apos;s worktree, once. Pushes to main are
          never offered here.
        </p>
      ) : null}
    </article>
  );
}

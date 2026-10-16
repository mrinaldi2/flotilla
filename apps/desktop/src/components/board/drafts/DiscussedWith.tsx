// Who the owner discusses a draft with (0.19.0): each invited bot with a
// Remove that asks first. Removing ends the discussion: the bot is told,
// can no longer see or change the draft, and its docked chat closes. What
// was said stays in the bot's chat.

import type { ReactElement } from "react";
import { useDialogKeys } from "../../../hooks/useReturnFocus";
import type { Bot } from "../../../protocol/entities";
import OverlayShell from "../../overlay/OverlayShell";

/** An invited bot as the drawer names it. */
export interface Invitee {
  readonly id: string;
  readonly name: string;
}

/** The invited bots, by name; one the app doesn't know is "a bot". */
export function invitees(invited: readonly string[], bots: readonly Bot[]): readonly Invitee[] {
  return invited.map((id) => ({ id, name: bots.find((b) => b.id === id)?.name ?? "a bot" }));
}

export function DiscussedWith(props: {
  readonly invitees: readonly Invitee[];
  /** The bots whose removal is on its way: their Remove waits for it. */
  readonly removing?: readonly string[];
  readonly onRemove: (invitee: Invitee) => void;
}): ReactElement | null {
  if (props.invitees.length === 0) {
    return null;
  }
  return (
    <section className="discussed-with" aria-label="Discussing the draft with">
      <span className="drawer-dim">Discussing with</span>
      <ul className="discussed-list">
        {props.invitees.map((invitee) => (
          <li key={invitee.id}>
            <span>{invitee.name}</span>
            <button
              type="button"
              className="btn btn-small"
              aria-label={`Remove ${invitee.name} from the discussion`}
              disabled={props.removing?.includes(invitee.id)}
              onClick={() => props.onRemove(invitee)}
            >
              Remove
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function RemoveDialog(props: {
  readonly itemId: string;
  readonly invitee: Invitee;
  readonly onRemove: () => void;
  readonly onCancel: () => void;
}): ReactElement {
  useDialogKeys(props.onCancel);
  const { name } = props.invitee;
  const title = `Remove ${name} from ${props.itemId}?`;
  return (
    <OverlayShell label={title} onClose={props.onCancel}>
      <div className="confirm-dialog">
        <h2 className="confirm-title">{title}</h2>
        <p className="confirm-body">
          {`The discussion ends: ${name} is told, and can no longer see or change the draft. What you said stays in ${name}'s chat.`}
        </p>
        <div className="confirm-actions">
          <button type="button" className="btn btn-small" onClick={props.onCancel}>
            Keep
          </button>
          <button type="button" className="btn btn-small btn-danger" onClick={props.onRemove}>
            Remove
          </button>
        </div>
      </div>
    </OverlayShell>
  );
}

// A lead's proposed board change, for the owner to rule on (0.19.0): the
// columns before and after, the rules it adds or drops, then Approve
// (applies it), Decline (with an optional note for the lead) or "Change
// it…", which opens the board's settings to make it differently.

import { useState } from "react";
import type { ReactElement } from "react";
import { kindChip } from "../settings/rules";
import type { FoundProposal } from "./useDecisionProposal";
import { proposalDiff } from "./proposalDiff";
import type { StripColumn } from "./proposalDiff";

const MARKS: Readonly<Record<NonNullable<StripColumn["mark"]>, string>> = {
  added: "+ added",
  removed: "− removed",
  changed: "~ changed",
};

function Strip(props: {
  readonly label: string;
  readonly columns: readonly StripColumn[];
}): ReactElement {
  return (
    <div className="proposal-strip">
      <span className="proposal-strip-label">{props.label}</span>
      {props.columns.length === 0 ? (
        <span className="proposal-strip-empty">No board yet</span>
      ) : (
        <ol aria-label={`Columns ${props.label.toLowerCase()}`}>
          {props.columns.map((c) => (
            <li key={c.key} className={c.mark === undefined ? "" : `proposal-${c.mark}`}>
              <span className="proposal-column-name">{c.name}</span>
              <span className="proposal-column-kind">{kindChip(c.kind)}</span>
              {c.mark === undefined ? null : <span className="proposal-mark">{MARKS[c.mark]}</span>}
            </li>
          ))}
        </ol>
      )}
    </div>
  );
}

interface ProposalReviewProps {
  readonly found: FoundProposal;
  readonly lead: string;
  readonly canApprove: boolean;
  readonly busy: boolean;
  readonly error: string | null;
  readonly onRule: (approve: boolean, note?: string) => void;
  /** Opens the board's settings, to make the change differently. */
  readonly onChange?: () => void;
}

export default function ProposalReview(props: ProposalReviewProps): ReactElement {
  const { proposal, board, columns } = props.found;
  const diff = proposalDiff(proposal, columns);
  const [note, setNote] = useState("");
  const disabled = !props.canApprove || props.busy;
  return (
    <section className="proposal-review" aria-label="Board change proposed">
      <p className="proposal-kicker">
        <span aria-hidden="true">⚙ </span>
        {`Board change proposed by ${props.lead}${board === undefined ? "" : ` · ${board.name}`}`}
      </p>
      <h1 className="cc-title">{proposal.summary}</h1>
      <p className="proposal-explain">
        The board's structure policy asks you to confirm the lead's changes. Nothing changes until
        you approve it.
      </p>
      <Strip label="Before" columns={diff.before} />
      <Strip label="After" columns={diff.after} />
      {diff.rules.length + diff.notes.length === 0 ? null : (
        <ul className="proposal-rules" aria-label="What else it changes">
          {[...diff.rules, ...diff.notes].map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}
      <label className="field proposal-note">
        <span className="field-label">Note for the lead (optional)</span>
        <input
          type="text"
          value={note}
          disabled={disabled}
          onChange={(e) => setNote(e.target.value)}
        />
      </label>
      {props.error === null ? null : (
        <p className="field-error" role="alert">
          {props.error}
        </p>
      )}
      <div className="proposal-actions">
        <button
          type="button"
          className="btn btn-small btn-good"
          disabled={disabled}
          onClick={() => props.onRule(true, note.trim() || undefined)}
        >
          <span aria-hidden="true">✓ </span>Approve
        </button>
        <button
          type="button"
          className="btn btn-small"
          disabled={disabled}
          onClick={() => props.onRule(false, note.trim() || undefined)}
        >
          <span aria-hidden="true">✗ </span>Decline
        </button>
        {props.onChange === undefined ? null : (
          <button type="button" className="btn btn-small" onClick={props.onChange}>
            Change it…
          </button>
        )}
      </div>
      {props.canApprove ? null : (
        <p className="settings-note">Approving a board change needs the owner's approve grant.</p>
      )}
    </section>
  );
}

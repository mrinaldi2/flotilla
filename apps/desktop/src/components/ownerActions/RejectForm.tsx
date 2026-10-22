// Reject…, opened: an optional reason, Cancel and Reject. Shared by the Run
// card and the push card (0.24.0).

import { useState } from "react";
import type { ReactElement } from "react";

export default function RejectForm(props: {
  readonly onCancel: () => void;
  readonly onReject: (reason: string) => void;
}): ReactElement {
  const [reason, setReason] = useState("");
  return (
    <div className="owner-action-buttons">
      <input
        className="owner-action-reason"
        aria-label="Why not (optional)"
        placeholder="Why not (optional)"
        value={reason}
        onChange={(e) => setReason(e.target.value)}
      />
      <button type="button" className="btn" onClick={props.onCancel}>
        Cancel
      </button>
      <button type="button" className="btn" onClick={() => props.onReject(reason)}>
        Reject
      </button>
    </div>
  );
}

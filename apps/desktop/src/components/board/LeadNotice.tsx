// "↻ Lead added column “QA” · 3h ago   View · ✕" (0.19.0): a change the
// lead made to the board shows over it until the owner dismisses it. View
// opens the board's settings. Dismissals are remembered on this computer.

import { X } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { BoardProposal } from "../../protocol/gen/flotilla/board/v1/requests_pb";
import { readStored, writeStored } from "../../storage";
import { leadChanges, noticeLine } from "./settings/lastChange";

const KEY = "board.dismissed.";

function loadDismissed(projectId: string): ReadonlySet<string> {
  try {
    const parsed: unknown = JSON.parse(readStored(KEY + projectId) ?? "[]");
    return new Set(
      Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === "string") : [],
    );
  } catch {
    return new Set();
  }
}

interface LeadNoticeProps {
  readonly projectId: string;
  readonly boardId: string;
  readonly proposals: readonly BoardProposal[];
  readonly who: (actor: string) => string;
  readonly now: number;
  readonly onView: () => void;
}

export default function LeadNotice(props: LeadNoticeProps): ReactElement | null {
  const [dismissed, setDismissed] = useState(() => loadDismissed(props.projectId));
  const shown = leadChanges(props.proposals, props.boardId).filter((p) => !dismissed.has(p.id));
  const newest = shown[0];
  if (newest === undefined) {
    return null;
  }
  const dismiss = (): void => {
    // Dismissing the newest dismisses every change before it too.
    const next = new Set([...dismissed, ...shown.map((p) => p.id)]);
    setDismissed(next);
    try {
      writeStored(KEY + props.projectId, JSON.stringify([...next].slice(-50)));
    } catch {
      // Not remembered: it shows again next launch.
    }
  };
  return (
    <div className="lead-notice" role="status">
      <span aria-hidden="true">↻ </span>
      <span className="lead-notice-text">
        {noticeLine(newest, props.who, props.now)}
        {shown.length > 1 ? ` (and ${shown.length - 1} more)` : ""}
      </span>
      <button type="button" className="chat-link" onClick={props.onView}>
        View
      </button>
      <button type="button" className="icon-btn" aria-label="Dismiss the notice" onClick={dismiss}>
        <X size={14} aria-hidden="true" />
      </button>
    </div>
  );
}

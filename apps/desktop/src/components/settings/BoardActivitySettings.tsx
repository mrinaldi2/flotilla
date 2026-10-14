import type { ReactElement } from "react";
import { BOARD_ACTIVITY_KINDS, setBoardActivity, useBoardActivityPrefs } from "../../prefs";
import type { BoardActivityKind } from "../../prefs";

const KINDS: Readonly<Record<BoardActivityKind, { label: string; help: string }>> = {
  moved: {
    label: "Card moves",
    help: "A bot, you on another device, or an automatic rule moved a card.",
  },
  assigned: { label: "Assignments", help: "A card was assigned, or a bot picked one up." },
  refused: { label: "Refused moves", help: "A move wasn't allowed, with why." },
  turns: { label: "Bots working", help: "A bot started or finished a turn on a card." },
  drafts: { label: "Draft updates", help: "A bot updated a draft you're discussing." },
};

/**
 * Which board activity drops a toast from the top of the window (0.19.0).
 * Stored with the other app preferences on this computer.
 */
export default function BoardActivitySettings(): ReactElement {
  const prefs = useBoardActivityPrefs();
  return (
    <>
      <h3 className="settings-subhead">Board activity</h3>
      <div className="settings-section">
        {BOARD_ACTIVITY_KINDS.map((kind) => {
          const id = `settings-board-activity-${kind}`;
          return (
            <div key={kind} className="settings-row">
              <div className="settings-row-text">
                <label className="settings-row-label" htmlFor={id}>
                  {KINDS[kind].label}
                </label>
                <div className="settings-row-help">{KINDS[kind].help}</div>
              </div>
              <label className="toggle" htmlFor={id} aria-label={KINDS[kind].label}>
                <input
                  id={id}
                  type="checkbox"
                  checked={prefs[kind]}
                  onChange={(event) => {
                    setBoardActivity(kind, event.target.checked);
                  }}
                />
                <span className="toggle-track" />
              </label>
            </div>
          );
        })}
      </div>
    </>
  );
}

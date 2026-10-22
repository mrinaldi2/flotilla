// One command on the Commands page (0.24.0): its status, the full command,
// whose it is and what for, Stop and Open bot, and its last output.

import { useState } from "react";
import type { ReactElement } from "react";
import { errText } from "../../util";
import CopyButton from "../CopyButton";
import type { CommandRow } from "./commandsText";
import { startedLabel, statusLabel, took } from "./commandsText";

interface CommandDetailProps {
  readonly row: CommandRow;
  readonly projectName: string;
  readonly computer: string | undefined;
  /** The owner may stop it: it runs in the background and they hold `approve`. */
  readonly canStop: boolean;
  /** Stops it; a refusal is thrown and shown here. */
  readonly onStop: () => Promise<void>;
  readonly onOpenBot: () => void;
  readonly onOpenItem: (itemId: string) => void;
  readonly now: number;
}

type WhoseProps = Pick<CommandDetailProps, "row" | "projectName" | "computer" | "onOpenItem">;

/** "ML Engineer · PhD · mac — for CASS-052", the card a link to the board. */
function Whose(props: WhoseProps): ReactElement {
  const { row } = props;
  const who = [row.bot_name, props.projectName, props.computer ?? ""]
    .filter((part) => part !== "")
    .join(" · ");
  let what: ReactElement | null = null;
  if (row.item_id !== undefined) {
    const itemId = row.item_id;
    what = (
      <>
        {" — for "}
        <button type="button" className="commands-link" onClick={() => props.onOpenItem(itemId)}>
          {itemId}
        </button>
      </>
    );
  } else if (row.task_id !== undefined) {
    what = <>{` — for task ${row.task_id}`}</>;
  }
  return (
    <p className="commands-detail-whose">
      {who}
      {what}
    </p>
  );
}

type ActionsProps = Pick<CommandDetailProps, "canStop" | "onStop" | "onOpenBot">;

/** Stop, confirmed once inline, and Open bot. */
function Actions(props: ActionsProps): ReactElement {
  const [confirming, setConfirming] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const stop = async (): Promise<void> => {
    setStopping(true);
    setError(null);
    try {
      await props.onStop();
      setConfirming(false);
    } catch (failure) {
      setError(errText(failure));
    } finally {
      setStopping(false);
    }
  };

  if (confirming) {
    return (
      <div className="commands-detail-actions" role="group" aria-label="Stop this command">
        <span className="commands-confirm">Stop this command? The bot is told.</span>
        <button
          type="button"
          className="btn btn-danger"
          disabled={stopping}
          onClick={() => void stop()}
        >
          {stopping ? "Stopping…" : "Stop command"}
        </button>
        <button
          type="button"
          className="btn"
          disabled={stopping}
          onClick={() => setConfirming(false)}
        >
          Cancel
        </button>
        {error === null ? null : <p className="field-error">{error}</p>}
      </div>
    );
  }
  return (
    <div className="commands-detail-actions">
      {props.canStop ? (
        <button type="button" className="btn btn-danger" onClick={() => setConfirming(true)}>
          Stop
        </button>
      ) : null}
      <button type="button" className="btn" onClick={props.onOpenBot}>
        Open bot
      </button>
    </div>
  );
}

/** The right-hand pane for the selected command. */
export default function CommandDetail({
  row,
  projectName,
  computer,
  canStop,
  onStop,
  onOpenBot,
  onOpenItem,
  now,
}: CommandDetailProps): ReactElement {
  const status = statusLabel(row);
  const output = row.output ?? "";
  return (
    <aside className="commands-detail" aria-label="Command detail">
      <div className="commands-detail-head">
        <div className="commands-detail-status">
          <span className={`commands-status commands-${row.status}`}>
            {`${status.glyph} ${status.text}${row.background ? " · background" : ""}`}
          </span>
          <span className="commands-detail-when">
            {`started ${startedLabel(row.started_at, now)} · ${took(row, now)}`}
          </span>
        </div>
        <div className="commands-detail-command">
          <code>{row.command}</code>
          <CopyButton text={row.command} label="Copy command" />
        </div>
        <Whose row={row} projectName={projectName} computer={computer} onOpenItem={onOpenItem} />
        <Actions canStop={canStop} onStop={onStop} onOpenBot={onOpenBot} />
      </div>
      <h3 className="commands-detail-label">Last output</h3>
      <pre className="commands-detail-output">{output === "" ? "No output yet." : output}</pre>
    </aside>
  );
}

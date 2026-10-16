import { SquareKanban } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { AddToast } from "../../app/useToasts";
import type { DaemonApi } from "../../protocol/api";
import { boardCall, PROTO_ENCODING } from "../../protocol/board";
import type { Bot, Project } from "../../protocol/entities";
import Board from "./Board";
import type { BoardProps } from "./Board";
import { useBoard } from "./useBoard";

interface BoardViewProps {
  readonly client: DaemonApi;
  readonly project: Project;
  /** This project's bots, for assignee names and avatars. */
  readonly bots: readonly Bot[];
  readonly connected: boolean;
  readonly canControl: boolean;
  readonly addToast: AddToast;
  /** The card whose drawer opens with the board ("Open on the board", UX-035 §4). */
  readonly openItem?: string;
  /** The board to show, from a link (0.19.0); the item's own board wins. */
  readonly openBoard?: string;
  /** Stories: open with a panel or sheet showing, at a pinned "now". */
  readonly initial?: BoardProps["initial"];
  readonly now?: number;
}

function Notice({
  title,
  text,
  action,
}: {
  readonly title: string;
  readonly text?: string;
  readonly action?: ReactElement;
}): ReactElement {
  return (
    <div className="empty-pane">
      <div className="empty-state">
        <SquareKanban className="coming-soon-icon" aria-hidden="true" />
        <h1>{title}</h1>
        {text === undefined ? null : <p>{text}</p>}
        {action}
      </div>
    </div>
  );
}

/**
 * A project with no board here: "No board yet". The owner starts it on
 * this computer, which becomes the home of all the project's boards
 * (H-037); a linked team's home is their choice, so the daemon never picks
 * one. Further boards are made from a preset with "New board…".
 */
function NoBoard(
  props: BoardViewProps & { readonly message: string; readonly refresh: () => void },
): ReactElement {
  const { client, project, message, refresh } = props;
  const [starting, setStarting] = useState(false);
  // Why the daemon refused to start it here: the board already lives on a
  // linked computer, or one can't confirm it has none (ARCH-R18 M1).
  const [refusal, setRefusal] = useState<string | null>(null);
  const owner = client.hasGrant("approve");
  const start = (): void => {
    setStarting(true);
    setRefusal(null);
    void (async () => {
      try {
        await boardCall(client, { case: "boardEnable", value: { projectId: project.id } }, "board");
        refresh();
      } catch (error) {
        setRefusal(error instanceof Error ? error.message : String(error));
      } finally {
        setStarting(false);
      }
    })();
  };
  return (
    <Notice
      title="No board yet"
      text={message}
      action={
        <>
          <p>
            {owner
              ? "Start it on one computer only: that computer becomes the home of the project's boards. It starts with To do, Doing, Review and Done, which you can change later."
              : "The owner starts it on the computer that will be its home."}
          </p>
          {owner ? (
            <button type="button" className="btn btn-small" disabled={starting} onClick={start}>
              Start the board on this computer
            </button>
          ) : null}
          {refusal === null ? null : (
            <p className="field-error" role="alert">
              {refusal}
            </p>
          )}
        </>
      }
    />
  );
}

/** The Board tab: the project's boards, columns and cards, live (H-018 §3). */
export default function BoardView(props: BoardViewProps): ReactElement {
  const { client, project, connected } = props;
  const supported = client.encodings.includes(PROTO_ENCODING);
  const { status, refresh, adopt } = useBoard(client, project.id, connected && supported);

  if (!supported) {
    return <Notice title="The board needs a newer Flotilla service" />;
  }
  if (status.kind === "loading") {
    return (
      <Notice title={connected ? "Loading the board…" : "Waiting for the Flotilla service…"} />
    );
  }
  if (status.kind === "failed") {
    if (status.code === "no_board") {
      return <NoBoard {...props} message={status.message} refresh={refresh} />;
    }
    return (
      <Notice
        title="Couldn't load the board"
        text={status.message}
        action={
          <button type="button" className="btn btn-small" onClick={refresh}>
            Try again
          </button>
        }
      />
    );
  }
  return <Board {...props} model={status.model} refresh={refresh} adopt={adopt} />;
}

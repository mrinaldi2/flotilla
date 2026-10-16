// The delete confirmation (0.19.0): what goes with the project, read from the
// service first ("This moves 1.6 GB to the Trash: …", and which linked
// computers are told), and a delete that waits for the project's name typed
// out. Cancel holds the focus and Escape cancels, so nothing confirms by
// accident.

import { useEffect, useState } from "react";
import type { ReactElement } from "react";
import { useEscapeKey } from "../../hooks/useEscapeKey";
import type { DaemonApi } from "../../protocol/api";
import type { Project } from "../../protocol/entities";
import { peersSentence, refusedSentence, trashSentence } from "../../protocol/projectTrash";
import type { ProjectDeletePreview } from "../../protocol/projectTrash";
import { errText } from "../../util";
import OverlayShell from "../overlay/OverlayShell";

interface DeleteProjectDialogProps {
  readonly client: DaemonApi;
  readonly project: Project;
  readonly botCount: number;
  readonly onConfirm: () => void;
  readonly onCancel: () => void;
}

type Preview =
  | { readonly kind: "loading" }
  | { readonly kind: "ready"; readonly preview: ProjectDeletePreview }
  | { readonly kind: "failed"; readonly why: string };

/** What happens to the bots, the part that needs no service answer. */
export function botsSentence(botCount: number): string {
  if (botCount === 0) {
    return "It holds no bots.";
  }
  const bots = botCount === 1 ? "bot is" : `${botCount} bots are`;
  return `Its ${bots} stopped and archived along with their conversations.`;
}

function diskLines(preview: Preview): readonly string[] {
  switch (preview.kind) {
    case "loading": {
      return ["Counting what it left on disk…"];
    }
    case "failed": {
      return [`What it left on disk couldn't be counted: ${preview.why}.`];
    }
    case "ready": {
      const p = preview.preview;
      return [trashSentence(p), peersSentence(p.peers), refusedSentence(p.refused)].filter(
        (line): line is string => line !== null,
      );
    }
  }
}

export default function DeleteProjectDialog(props: DeleteProjectDialogProps): ReactElement {
  const { client, project, onCancel } = props;
  const [preview, setPreview] = useState<Preview>({ kind: "loading" });
  const [typed, setTyped] = useState("");

  useEffect(() => {
    let live = true;
    const load = async (): Promise<void> => {
      let next: Preview;
      try {
        const reply = await client.request(
          { type: "project_delete_preview", project_id: project.id },
          "project_delete_preview",
        );
        next = { kind: "ready", preview: reply.preview };
      } catch (err) {
        next = { kind: "failed", why: errText(err) };
      }
      if (live) {
        setPreview(next);
      }
    };
    void load();
    return () => {
      live = false;
    };
  }, [client, project.id]);

  useEscapeKey(onCancel);

  const title = `Delete ${project.name}?`;
  const named = typed.trim() === project.name;
  return (
    <OverlayShell label={title} onClose={onCancel}>
      <div className="confirm-dialog">
        <h2 className="confirm-title">{title}</h2>
        <p className="confirm-body">{botsSentence(props.botCount)}</p>
        {diskLines(preview).map((line) => (
          <p key={line} className="confirm-body">
            {line}
          </p>
        ))}
        <label className="field">
          <span className="field-hint">
            Type <strong className="mono">{project.name}</strong> to delete it
          </span>
          <input
            type="text"
            value={typed}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => {
              setTyped(event.target.value);
            }}
          />
        </label>
        <div className="confirm-actions">
          <button type="button" className="btn btn-small" autoFocus onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-small btn-danger"
            disabled={!named}
            onClick={props.onConfirm}
          >
            Delete project
          </button>
        </div>
      </div>
    </OverlayShell>
  );
}

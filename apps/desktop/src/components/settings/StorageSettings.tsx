// Settings → Storage (0.19.0): Clean up deleted projects. Projects deleted
// before this version kept their folders, their bots' session history and
// their git caches on disk; the service finds them, the owner reads what
// would move, and only a confirmation moves it to the trash.

import { useCallback, useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import { sizeText } from "../../protocol/cleanup";
import type { NotifyLevel } from "../../protocol/entities";
import { deletedSentence } from "../../protocol/projectTrash";
import type { DeletedProjectsPreview } from "../../protocol/projectTrash";
import { errText } from "../../util";
import ConfirmDialog from "../overlay/ConfirmDialog";

interface StorageSettingsProps {
  readonly client: DaemonApi;
  readonly connected: boolean;
  readonly canControl: boolean;
  readonly onToast: (level: NotifyLevel, title: string, body: string) => void;
}

/** Linked computers that haven't confirmed trashing their copy. */
function LinkedWaiting(props: { readonly preview: DeletedProjectsPreview }): ReactElement | null {
  if (props.preview.linked.length === 0) {
    return null;
  }
  return (
    <ul>
      {props.preview.linked.map((l) => (
        <li key={`${l.project_name}:${l.peer_name}`} className="field-hint">
          {l.project_name} on {l.peer_name}:{" "}
          {l.state === "unconfirmed" ? "didn't confirm" : "not told yet"}
        </li>
      ))}
    </ul>
  );
}

export default function StorageSettings(props: StorageSettingsProps): ReactElement {
  const { client, connected, canControl, onToast } = props;
  const [preview, setPreview] = useState<DeletedProjectsPreview | null>(null);
  const [busy, setBusy] = useState(false);

  const look = useCallback(async (): Promise<void> => {
    setBusy(true);
    try {
      const reply = await client.request(
        { type: "deleted_projects_preview" },
        "deleted_projects_preview",
      );
      setPreview(reply.preview);
      if (reply.preview.projects.length === 0) {
        onToast("info", "Nothing to clean up", "Deleted projects left nothing on this computer.");
      }
    } catch (err) {
      onToast("error", "Couldn't look for deleted projects", errText(err));
    } finally {
      setBusy(false);
    }
  }, [client, onToast]);

  const clean = useCallback(async (): Promise<void> => {
    setBusy(true);
    try {
      const done = await client.request(
        { type: "deleted_projects_cleanup" },
        "deleted_projects_cleaned",
      );
      const level = done.skipped.length === 0 ? "info" : "warn";
      onToast(level, "Deleted projects cleaned up", done.summary);
    } catch (err) {
      onToast("error", "Couldn't clean up deleted projects", errText(err));
    } finally {
      setBusy(false);
    }
  }, [client, onToast]);

  const confirming = preview !== null && preview.projects.length > 0;
  return (
    <div className="settings-section">
      <div className="panel">
        <h3 className="panel-title">Deleted projects</h3>
        <p className="field-hint">
          Deleting a project moves what it left on disk to the trash. Projects deleted before kept
          their folders, their bots&apos; session history and their git caches; this finds them and
          shows what would move before anything does.
        </p>
        {preview === null ? null : <LinkedWaiting preview={preview} />}
        <div className="panel-actions">
          <button
            type="button"
            className="btn"
            disabled={!connected || !canControl || busy}
            onClick={() => {
              void look();
            }}
          >
            {busy ? "Looking…" : "Clean up deleted projects"}
          </button>
        </div>
      </div>
      {confirming ? (
        <ConfirmDialog
          title={`Move ${sizeText(preview.total_bytes)} to the ${preview.bin}?`}
          body={deletedSentence(preview)}
          confirmLabel="Move to the trash"
          onConfirm={() => {
            setPreview(null);
            void clean();
          }}
          onCancel={() => {
            setPreview(null);
          }}
        />
      ) : null}
    </div>
  );
}

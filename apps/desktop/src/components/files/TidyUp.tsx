// Tidy up (0.22.0, `artifacts_v2`): the shared files no card uses and the
// owner hasn't kept, for the owner to keep (`artifact_keep`), attach to the
// card they belong to, or remove to the trash (`artifact_trash`, after a
// confirm). Nothing is moved or deleted by itself.

import { useState } from "react";
import type { FormEvent, ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import { boardCall } from "../../protocol/board";
import type { Artifact } from "../../protocol/chat";
import { errText } from "../../util";
import ConfirmDialog from "../overlay/ConfirmDialog";
import { sizeLabel } from "./ArtifactRow";

/** The shared files no card uses that the owner hasn't kept, or settled here. */
function looseFiles(
  artifacts: readonly Artifact[],
  settled: ReadonlySet<string>,
): readonly Artifact[] {
  return artifacts.filter(
    (a) =>
      (a.group ?? "shared") === "shared" &&
      (a.used_by ?? []).length === 0 &&
      a.kept !== true &&
      !settled.has(a.rel),
  );
}

function AttachForm(props: {
  readonly client: DaemonApi;
  readonly artifact: Artifact;
  readonly onAttached: (card: string) => void;
  readonly onCancel: () => void;
}): ReactElement {
  const [card, setCard] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const submit = (event: FormEvent): void => {
    event.preventDefault();
    const id = card.trim();
    setBusy(true);
    setError(null);
    void (async () => {
      try {
        await boardCall(
          props.client,
          { case: "itemAttach", value: { id, artifact: props.artifact.rel } },
          "item",
        );
        props.onAttached(id);
      } catch (failure) {
        setError(errText(failure));
        setBusy(false);
      }
    })();
  };
  return (
    <form className="files-attach" onSubmit={submit}>
      <input
        type="text"
        aria-label={`Card to attach ${props.artifact.name} to`}
        placeholder="Card, e.g. ASL-4"
        value={card}
        autoFocus
        onChange={(e) => setCard(e.target.value)}
      />
      <button type="submit" className="btn btn-small" disabled={busy || card.trim() === ""}>
        Attach
      </button>
      <button type="button" className="btn btn-small" onClick={props.onCancel}>
        Cancel
      </button>
      {error === null ? null : (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

/** What this connection may do to a loose file. */
interface Allowed {
  /** The control grant: attach it to a card. */
  readonly attach: boolean;
  /** The approve grant: keep it or remove it. */
  readonly settle: boolean;
}

function LooseActions(props: {
  readonly client: DaemonApi;
  readonly artifact: Artifact;
  readonly allowed: Allowed;
  readonly onKeep: () => void;
  readonly onRemove: () => void;
  readonly onAttached: (card: string) => void;
}): ReactElement | null {
  const [attaching, setAttaching] = useState(false);
  const { allowed } = props;
  if (attaching) {
    return (
      <AttachForm
        client={props.client}
        artifact={props.artifact}
        onAttached={props.onAttached}
        onCancel={() => setAttaching(false)}
      />
    );
  }
  if (!allowed.attach && !allowed.settle) {
    return null;
  }
  return (
    <div className="files-loose-actions">
      {allowed.settle ? (
        <button type="button" className="btn btn-small" onClick={props.onKeep}>
          Keep
        </button>
      ) : null}
      {allowed.attach ? (
        <button type="button" className="btn btn-small" onClick={() => setAttaching(true)}>
          Attach to a card…
        </button>
      ) : null}
      {allowed.settle ? (
        <button type="button" className="btn btn-small" onClick={props.onRemove}>
          Remove…
        </button>
      ) : null}
    </div>
  );
}

/** Keeps, removes and attaches loose files, and says what happened. */
function useTidy(client: DaemonApi, projectId: string, onChanged: () => void) {
  // Settled here: out of the list before the list is read again.
  const [settled, setSettled] = useState<ReadonlySet<string>>(new Set());
  const [note, setNote] = useState<{ readonly text: string; readonly failed: boolean } | null>(
    null,
  );
  const done = (a: Artifact, text: string): void => {
    setSettled((s) => new Set(s).add(a.rel));
    setNote({ text, failed: false });
    onChanged();
  };
  const run = async (a: Artifact, send: () => Promise<unknown>, text: string): Promise<void> => {
    try {
      await send();
      done(a, text);
    } catch (failure) {
      setNote({ text: `${a.name}: ${errText(failure)}`, failed: true });
    }
  };
  const keep = (a: Artifact): Promise<void> =>
    run(
      a,
      () =>
        client.request(
          { type: "artifact_keep", project_id: projectId, path: a.rel, keep: true },
          "artifact_kept",
        ),
      `Kept ${a.name}.`,
    );
  const remove = (a: Artifact): Promise<void> =>
    run(
      a,
      () =>
        client.request(
          { type: "artifact_trash", project_id: projectId, path: a.rel },
          "artifact_trashed",
        ),
      `Moved ${a.name} to the trash.`,
    );
  const attached = (a: Artifact, card: string): void => done(a, `Attached ${a.name} to ${card}.`);
  return { settled, note, keep, remove, attached };
}

export default function TidyUp(props: {
  readonly client: DaemonApi;
  readonly projectId: string;
  /** The shared files read so far. */
  readonly artifacts: readonly Artifact[];
  readonly allowed: Allowed;
  readonly onSelect: (path: string) => void;
  /** A file was kept, removed or attached: the list reads again. */
  readonly onChanged: () => void;
}): ReactElement {
  const tidy = useTidy(props.client, props.projectId, props.onChanged);
  const [removing, setRemoving] = useState<Artifact | null>(null);
  const loose = looseFiles(props.artifacts, tidy.settled);
  return (
    <div className="files-tidy">
      <p className="files-tidy-intro">
        Shared files no card uses. Keep the ones the project needs, attach each to its card, or
        remove it.
      </p>
      {tidy.note === null ? null : (
        <p className="files-tidy-note" role={tidy.note.failed ? "alert" : "status"}>
          {tidy.note.text}
        </p>
      )}
      {loose.length === 0 ? (
        <div className="files-empty">Every shared file is on a card or kept.</div>
      ) : (
        <ul className="files-list">
          {loose.map((a) => (
            <li key={a.path} className="files-item files-loose">
              <button
                type="button"
                className="files-row"
                title={a.rel}
                onClick={() => props.onSelect(a.path)}
              >
                <span className="files-name">{a.title ?? a.name}</span>
                <span className="files-meta">
                  {a.rel} · {sizeLabel(a.size)}
                </span>
              </button>
              <LooseActions
                client={props.client}
                artifact={a}
                allowed={props.allowed}
                onKeep={() => void tidy.keep(a)}
                onRemove={() => setRemoving(a)}
                onAttached={(card) => tidy.attached(a, card)}
              />
            </li>
          ))}
        </ul>
      )}
      {removing === null ? null : (
        <ConfirmDialog
          title={`Remove ${removing.name}?`}
          body="It moves to the trash on the computer that holds the project, where you can still restore it."
          confirmLabel="Move to trash"
          onConfirm={() => {
            const a = removing;
            setRemoving(null);
            void tidy.remove(a);
          }}
          onCancel={() => setRemoving(null)}
        />
      )}
    </div>
  );
}

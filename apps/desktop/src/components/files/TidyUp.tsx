// Tidy up (0.22.0, `artifacts_v2`): the shared files no card uses, for the
// owner to keep, or attach to the card they belong to. Nothing is moved or
// deleted by itself. Kept files are remembered on this computer and leave
// the list.

import { useState } from "react";
import type { FormEvent, ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import { boardCall } from "../../protocol/board";
import type { Artifact } from "../../protocol/chat";
import { readStored, writeStored } from "../../storage";
import { errText } from "../../util";
import { sizeLabel } from "./ArtifactRow";

function keptName(projectId: string): string {
  return `artifacts.kept.${projectId}`;
}

/** The files kept here, by `rel`. Storage can be unavailable: nothing is kept then. */
function readKept(projectId: string): ReadonlySet<string> {
  try {
    const raw = readStored(keptName(projectId));
    const parsed: unknown = raw === null ? [] : JSON.parse(raw);
    return new Set(
      Array.isArray(parsed) ? parsed.filter((x): x is string => typeof x === "string") : [],
    );
  } catch {
    return new Set();
  }
}

function writeKept(projectId: string, kept: ReadonlySet<string>): void {
  try {
    writeStored(keptName(projectId), JSON.stringify([...kept]));
  } catch {
    // Kept for this session only.
  }
}

/** The shared files no card uses that weren't kept. */
function looseFiles(
  artifacts: readonly Artifact[],
  kept: ReadonlySet<string>,
): readonly Artifact[] {
  return artifacts.filter(
    (a) => (a.group ?? "shared") === "shared" && (a.used_by ?? []).length === 0 && !kept.has(a.rel),
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

function LooseRow(props: {
  readonly client: DaemonApi;
  readonly artifact: Artifact;
  readonly canEdit: boolean;
  readonly onKeep: () => void;
  readonly onAttached: (card: string) => void;
  readonly onSelect: (path: string) => void;
}): ReactElement {
  const { artifact: a } = props;
  const [attaching, setAttaching] = useState(false);
  return (
    <li className="files-item files-loose">
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
      {!props.canEdit ? null : attaching ? (
        <AttachForm
          client={props.client}
          artifact={a}
          onAttached={props.onAttached}
          onCancel={() => setAttaching(false)}
        />
      ) : (
        <div className="files-loose-actions">
          <button type="button" className="btn btn-small" onClick={props.onKeep}>
            Keep
          </button>
          <button type="button" className="btn btn-small" onClick={() => setAttaching(true)}>
            Attach to a card…
          </button>
        </div>
      )}
    </li>
  );
}

export default function TidyUp(props: {
  readonly client: DaemonApi;
  readonly projectId: string;
  /** The shared files read so far. */
  readonly artifacts: readonly Artifact[];
  /** The control grant: attaches files to cards. */
  readonly canEdit: boolean;
  readonly onSelect: (path: string) => void;
  /** A file was attached: the list reads again, with its card. */
  readonly onChanged: () => void;
}): ReactElement {
  const { projectId } = props;
  const [kept, setKept] = useState(() => readKept(projectId));
  const [note, setNote] = useState<string | null>(null);
  const loose = looseFiles(props.artifacts, kept);
  const keep = (a: Artifact): void => {
    const next = new Set(kept).add(a.rel);
    setKept(next);
    writeKept(projectId, next);
    setNote(`Kept ${a.name}.`);
  };
  return (
    <div className="files-tidy">
      <p className="files-tidy-intro">
        Shared files no card uses. Keep the ones the project needs, or attach each to its card.
      </p>
      {note === null ? null : (
        <p className="files-tidy-note" role="status">
          {note}
        </p>
      )}
      {loose.length === 0 ? (
        <div className="files-empty">Every shared file is on a card or kept.</div>
      ) : (
        <ul className="files-list">
          {loose.map((a) => (
            <LooseRow
              key={a.path}
              client={props.client}
              artifact={a}
              canEdit={props.canEdit}
              onSelect={props.onSelect}
              onKeep={() => keep(a)}
              onAttached={(card) => {
                setNote(`Attached ${a.name} to ${card}.`);
                props.onChanged();
              }}
            />
          ))}
        </ul>
      )}
    </div>
  );
}

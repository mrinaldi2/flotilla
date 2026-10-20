// One artifact in the list: its title or name, where it is, its size and
// age, who made it, and (0.22.0) the cards that use it.

import type { ReactElement } from "react";
import type { Artifact } from "../../protocol/chat";
import { fmtTimestamp } from "../../util";
import CardLink from "../cards/CardLink";
import CreatedBy from "./CreatedBy";

export function sizeLabel(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${Math.round(bytes / 1024)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** "on ASL-4 ASL-7": the cards that attach or link it. */
function OnCards({ ids }: { readonly ids: readonly string[] }): ReactElement | null {
  if (ids.length === 0) {
    return null;
  }
  return (
    <span className="files-cards">
      <span className="files-cards-word">on</span>
      {ids.map((id) => (
        <span key={id} className="files-card-chip">
          <CardLink id={id} />
        </span>
      ))}
    </span>
  );
}

export default function ArtifactRow(props: {
  readonly artifact: Artifact;
  readonly onSelect: (path: string) => void;
}): ReactElement {
  const { artifact } = props;
  return (
    <li className="files-item">
      <button
        type="button"
        className="files-row"
        title={artifact.rel}
        onClick={() => {
          props.onSelect(artifact.path);
        }}
      >
        <span className="files-name">{artifact.title ?? artifact.name}</span>
        <span className="files-meta">
          {artifact.title === undefined ? artifact.rel : artifact.name} · {sizeLabel(artifact.size)}
          {artifact.modified == null ? "" : ` · ${fmtTimestamp(artifact.modified)}`}
        </span>
        {artifact.created_by === undefined ? null : <CreatedBy creator={artifact.created_by} />}
      </button>
      <OnCards ids={artifact.used_by ?? []} />
    </li>
  );
}

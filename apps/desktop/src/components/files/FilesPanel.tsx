// The project's artifacts beside a bot, newest first, a page at a time, and
// a preview of one. With `artifacts_v2` (0.22.0) they are grouped by kind:
// Shared (files no single card owns), Uploads (the owner's) and Checks
// (check logs), each with the cards that use it, and Tidy up lists the
// shared files no card uses.

import { useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { ArtifactGroup } from "../../protocol/chat";
import { ARTIFACTS_V2 } from "../../protocol/chat";
import type { Bot } from "../../protocol/entities";
import ArtifactRow from "./ArtifactRow";
import FilePreview from "./FilePreview";
import TidyUp from "./TidyUp";
import { useArtifactPages } from "./useArtifactPages";

interface FilesPanelProps {
  readonly client: DaemonApi;
  readonly bot: Bot;
  readonly connected: boolean;
  /** The file being previewed, or null for the list. */
  readonly selected: string | null;
  readonly onSelect: (path: string | null) => void;
  /** The control grant: Tidy up attaches files to cards. */
  readonly canControl?: boolean;
}

type View = ArtifactGroup | "tidy";

const VIEWS: readonly (readonly [View, string])[] = [
  ["shared", "Shared"],
  ["uploads", "Uploads"],
  ["checks", "Checks"],
  ["tidy", "Tidy up"],
];

const EMPTY: Readonly<Record<ArtifactGroup, string>> = {
  shared: "No shared files in this project yet.",
  uploads: "You haven't uploaded anything yet.",
  checks: "No check logs yet.",
};

function Groups(props: {
  readonly view: View;
  readonly onView: (view: View) => void;
}): ReactElement {
  return (
    <div className="files-groups" role="tablist" aria-label="Artifacts">
      {VIEWS.map(([id, label]) => (
        <button
          key={id}
          type="button"
          role="tab"
          aria-selected={props.view === id}
          className={`files-group${props.view === id ? " files-group-on" : ""}`}
          onClick={() => props.onView(id)}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

/** The group a view reads: Tidy up reads the shared files, and lists the loose ones. */
function groupOf(grouped: boolean, view: View): ArtifactGroup | undefined {
  if (!grouped) {
    return undefined;
  }
  return view === "tidy" ? "shared" : view;
}

function Files(props: {
  readonly pages: ReturnType<typeof useArtifactPages>;
  readonly group: ArtifactGroup | undefined;
  readonly onSelect: (path: string) => void;
}): ReactElement {
  const { pages, group } = props;
  const empty = group === undefined ? "No artifacts in this project yet." : EMPTY[group];
  return (
    <>
      {pages.artifacts.length === 0 && pages.error === null && !pages.loading ? (
        <div className="files-empty">{empty}</div>
      ) : null}
      <ul className="files-list">
        {pages.artifacts.map((artifact) => (
          <ArtifactRow key={artifact.path} artifact={artifact} onSelect={props.onSelect} />
        ))}
      </ul>
    </>
  );
}

function ArtifactList(props: FilesPanelProps): ReactElement {
  const { client, bot, connected, onSelect } = props;
  const grouped = client.capabilities.includes(ARTIFACTS_V2);
  const [view, setView] = useState<View>("shared");
  const group = groupOf(grouped, view);
  const pages = useArtifactPages(client, bot.project_id, connected, group);
  return (
    <div className="files-panel">
      <div className="files-head">
        <span className="panel-title">Artifacts</span>
        <button type="button" className="btn btn-small" onClick={pages.reload}>
          Refresh
        </button>
      </div>
      {grouped ? <Groups view={view} onView={setView} /> : null}
      {pages.error === null ? null : <div className="chat-note chat-error">{pages.error}</div>}
      {view === "tidy" ? (
        <TidyUp
          key={bot.project_id}
          client={client}
          projectId={bot.project_id}
          artifacts={pages.artifacts}
          canEdit={props.canControl ?? false}
          onSelect={onSelect}
          onChanged={pages.reload}
        />
      ) : (
        <Files pages={pages} group={group} onSelect={onSelect} />
      )}
      {pages.more === undefined ? null : (
        <button type="button" className="btn btn-small files-more" onClick={pages.more}>
          Show older
        </button>
      )}
    </div>
  );
}

export default function FilesPanel(props: FilesPanelProps): ReactElement {
  const { client, bot, selected, onSelect } = props;
  if (selected !== null) {
    return (
      <FilePreview
        client={client}
        botId={bot.id}
        path={selected}
        onBack={() => {
          onSelect(null);
        }}
      />
    );
  }
  return <ArtifactList {...props} />;
}

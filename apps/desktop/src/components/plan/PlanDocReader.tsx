// A plan document read in place (0.21.0): the file from the project's
// repository at its commit, through `plan_doc_read`, shown by the card
// documents' own preview (Markdown, a sandboxed HTML page with its relative
// files, text, PDFs, images). Once approved, "Changes since approval" shows
// the diff from the approved commit.

import { create } from "@bufbuild/protobuf";
import { useEffect, useMemo, useState } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../protocol/board";
import { ItemAttachmentSchema } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { readPlanDoc } from "../../protocol/plan";
import { errText } from "../../util";
import DocumentPreview from "../board/attachments/DocumentPreview";
import type { Reader } from "../board/attachments/preview";
import { fileName } from "./planText";

interface PlanDocReaderProps {
  readonly api: BoardApi;
  readonly projectId: string;
  readonly path: string;
  /** The commit to read; the default branch's head without one. */
  readonly sha?: string;
  /** The commit the owner approved it at: offers "Changes since approval". */
  readonly approvedSha?: string;
  /** The tab it opens on (stories). */
  readonly initialTab?: "doc" | "changes";
}

type Diff =
  | { readonly status: "loading" }
  | { readonly status: "ready"; readonly diff: string }
  | { readonly status: "failed"; readonly message: string };

/** One diff line's class: added, removed, a hunk, or context. */
function diffLineClass(line: string): string {
  if (line.startsWith("@@")) {
    return "plan-diff-hunk";
  }
  if (line.startsWith("+") && !line.startsWith("+++")) {
    return "plan-diff-add";
  }
  if (line.startsWith("-") && !line.startsWith("---")) {
    return "plan-diff-del";
  }
  return "";
}

function DiffView(props: PlanDocReaderProps & { readonly from: string }): ReactElement {
  const { api, projectId, path, sha, from } = props;
  const [state, setState] = useState<Diff>({ status: "loading" });
  useEffect(() => {
    let live = true;
    void (async () => {
      try {
        const file = await readPlanDoc(api, projectId, path, { sha, diffFrom: from });
        if (live) {
          setState({ status: "ready", diff: file.diff ?? "" });
        }
      } catch (failure) {
        if (live) {
          setState({ status: "failed", message: errText(failure) });
        }
      }
    })();
    return () => {
      live = false;
    };
  }, [api, projectId, path, sha, from]);
  if (state.status === "loading") {
    return <span className="lightbox-note">Loading…</span>;
  }
  if (state.status === "failed") {
    return <span className="lightbox-note">{state.message}</span>;
  }
  if (state.diff.trim() === "") {
    return <p className="plan-empty">No changes since it was approved.</p>;
  }
  return (
    <pre className="plan-diff" aria-label={`Changes to ${fileName(path)} since approval`}>
      {state.diff.split("\n").map((line, index) => (
        // The diff's lines don't move: their place is their identity.
        // oxlint-disable-next-line react/no-array-index-key
        <span key={index} className={diffLineClass(line)}>
          {line}
          {"\n"}
        </span>
      ))}
    </pre>
  );
}

export default function PlanDocReader(props: PlanDocReaderProps): ReactElement {
  const { api, projectId, path, sha, approvedSha } = props;
  const [tab, setTab] = useState(props.initialTab ?? "doc");
  const name = fileName(path);
  const attachment = useMemo(
    () => create(ItemAttachmentSchema, { id: `${path}@${sha ?? "head"}`, kind: "doc", name, path }),
    [path, sha, name],
  );
  const reader = useMemo(
    (): Reader => (file) => readPlanDoc(api, projectId, file, { sha }),
    [api, projectId, sha],
  );
  const changes = approvedSha !== undefined && approvedSha !== sha;
  const shown = changes ? tab : "doc";
  return (
    <div className="plan-doc">
      <div className="plan-doc-tabs" role="tablist" aria-label="View">
        <button
          type="button"
          role="tab"
          aria-selected={shown === "doc"}
          onClick={() => setTab("doc")}
        >
          {name}
        </button>
        {changes ? (
          <button
            type="button"
            role="tab"
            aria-selected={shown === "changes"}
            onClick={() => setTab("changes")}
          >
            Changes since approval
          </button>
        ) : null}
      </div>
      <p className="plan-doc-path mono">
        {path}
        {sha === undefined ? "" : ` @ ${sha}`}
      </p>
      <div className="plan-doc-body" role="tabpanel">
        {shown === "changes" && approvedSha !== undefined ? (
          <DiffView key={`${sha ?? ""}-${approvedSha}`} {...props} from={approvedSha} />
        ) : (
          <DocumentPreview api={api} attachment={attachment} readArtifact={reader} />
        )}
      </div>
    </div>
  );
}

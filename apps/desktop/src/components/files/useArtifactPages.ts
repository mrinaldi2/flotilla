// A project's artifacts, a page at a time: `list_artifacts` with `limit`,
// then `next_before` for the page after. With `artifacts_v2` (0.22.0) the
// list is one group's: shared files, the owner's uploads or check logs.

import { useCallback, useEffect, useState } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { Artifact, ArtifactGroup } from "../../protocol/chat";
import { errText } from "../../util";

/** Files a page reads. */
const PAGE = 100;

export interface ArtifactPages {
  readonly artifacts: readonly Artifact[];
  readonly error: string | null;
  readonly loading: boolean;
  /** Reads the next page; absent once there is none. */
  readonly more?: () => void;
  /** Reads the first page again. */
  readonly reload: () => void;
}

interface Pages {
  readonly artifacts: readonly Artifact[];
  readonly next: string | null;
  readonly error: string | null;
  readonly loading: boolean;
}

const START: Pages = { artifacts: [], next: null, error: null, loading: true };

export function useArtifactPages(
  client: DaemonApi,
  projectId: string,
  connected: boolean,
  /** Absent with a daemon that lists every file in one list. */
  group: ArtifactGroup | undefined,
): ArtifactPages {
  const [pages, setPages] = useState<Pages>(START);
  // `before` of the page asked for; `reads` counts the asks, so a reload reads again.
  const [ask, setAsk] = useState<{ readonly before?: string; readonly reads: number }>({
    reads: 0,
  });

  useEffect(() => {
    if (!connected) {
      return undefined;
    }
    let live = true;
    const before = ask.before;
    void (async () => {
      try {
        const reply = await client.request(
          { type: "list_artifacts", project_id: projectId, group, limit: PAGE, before },
          "artifacts",
        );
        if (live) {
          setPages((p) => ({
            artifacts:
              before === undefined ? reply.artifacts : [...p.artifacts, ...reply.artifacts],
            next: reply.has_more === true ? (reply.next_before ?? null) : null,
            error: null,
            loading: false,
          }));
        }
      } catch (failure) {
        if (live) {
          setPages((p) => ({ ...p, error: errText(failure), loading: false }));
        }
      }
    })();
    return () => {
      live = false;
    };
  }, [client, projectId, connected, group, ask]);

  // Another group starts from its first page.
  const [shownGroup, setShownGroup] = useState(group);
  if (shownGroup !== group) {
    setShownGroup(group);
    setPages(START);
    setAsk((a) => ({ reads: a.reads + 1 }));
  }

  const reload = useCallback((): void => {
    setAsk((a) => ({ reads: a.reads + 1 }));
  }, []);
  const next = pages.next;
  const more =
    next === null || pages.loading
      ? undefined
      : (): void => {
          setPages((p) => ({ ...p, loading: true }));
          setAsk((a) => ({ before: next, reads: a.reads + 1 }));
        };
  return { artifacts: pages.artifacts, error: pages.error, loading: pages.loading, more, reload };
}

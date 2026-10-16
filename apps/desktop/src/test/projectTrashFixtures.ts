// What deleting a project would move, for tests and stories (0.19.0).

import type { DeletedProjectsPreview, ProjectDeletePreview } from "../protocol/projectTrash";

/** Acme: 1.6 GB, most of it its bots' session history, linked with win. */
export const DELETE_PREVIEW: ProjectDeletePreview = {
  project_id: "p1",
  project_name: "Acme",
  deleted_at: null,
  groups: [
    { group: "project_files", label: "project files", bytes: 380_000_000, count: 1, items: [] },
    {
      group: "session_history",
      label: "bot session history",
      bytes: 1_217_000_000,
      count: 3,
      items: [],
    },
    { group: "git_cache", label: "git cache", bytes: 3_000_000, count: 1, items: [] },
  ],
  total_bytes: 1_600_000_000,
  refused: [],
  bin: "Trash",
  peers: [{ peer_id: "peer-1", name: "win", online: false }],
};

/** Two projects deleted before 0.19, and a computer that didn't confirm. */
export const DELETED_PREVIEW: DeletedProjectsPreview = {
  projects: [
    { ...DELETE_PREVIEW, project_name: "Old", total_bytes: 1_200_000_000 },
    { ...DELETE_PREVIEW, project_id: "p2", project_name: "Other", total_bytes: 200_000_000 },
  ],
  total_bytes: 1_400_000_000,
  bin: "Trash",
  linked: [{ project_name: "Old", peer_name: "win", state: "unconfirmed" }],
};

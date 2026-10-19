// A project's artifacts by kind (0.22.0, `artifacts_v2`), for tests and
// stories: shared files (one on two cards, two loose, one kept), an upload
// and a check log.

import type { Artifact } from "../protocol/chat";

const ROOT = "/home/flotilla/projects/saints/artifacts";

function file(rel: string, over: Partial<Artifact>): Artifact {
  return {
    path: `${ROOT}/${rel}`,
    rel,
    name: rel.slice(rel.lastIndexOf("/") + 1),
    size: 2048,
    modified: "2024-05-01T08:00:00Z",
    mime: "text/plain",
    group: "shared",
    used_by: [],
    ...over,
  };
}

export const GROUPED_ARTIFACTS: readonly Artifact[] = [
  file("data/saints.json", {
    title: "Saints dataset",
    size: 412_000,
    mime: "application/json",
    used_by: ["ASL-4", "ASL-7"],
    created_by: { bot_id: "b2", name: "Researcher", avatar: "icon:book", via: "wrote" },
  }),
  file("notes-0917.md", {
    mime: "text/markdown",
    modified: "2024-04-17T16:20:00Z",
    created_by: { bot_id: "b-lead", name: "Lead", avatar: "icon:orbit", via: "wrote" },
  }),
  file("runs/frames-run3.csv", { size: 1_830_000, mime: "text/csv" }),
  file("style-guide.md", { mime: "text/markdown", kept: true }),
  file("uploads/calendar-2026.pdf", {
    group: "uploads",
    size: 920_000,
    mime: "application/pdf",
    used_by: ["ASL-4"],
    created_by: { name: "you", avatar: "", via: "upload" },
  }),
  file("checks/9b2f0c1a4e5d-build.log", { group: "checks", size: 64_000 }),
];

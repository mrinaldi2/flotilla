// Deleting a project moves what it left on disk to the trash (0.19.0): the
// preview the delete confirmation shows, and the cleanup of projects deleted
// before. The owner's alone: the app's ticket or a paired device.

import { sizeText } from "./cleanup";

type TrashGroupKind =
  | "project_files"
  | "session_history"
  | "git_cache"
  | "worktrees"
  | "saved_work"
  | (string & Record<never, never>);

interface TrashGroup {
  readonly group: TrashGroupKind;
  /** What the owner calls it: "bot session history". */
  readonly label: string;
  readonly bytes: number;
  readonly count: number;
  readonly items: readonly { readonly path: string; readonly bytes: number }[];
}

/** Something found that is left alone, and why. */
export interface TrashRefusal {
  readonly path: string;
  readonly reason: string;
}

interface TrashLeftovers {
  readonly project_id: string;
  readonly project_name: string;
  readonly deleted_at: string | null;
  readonly groups: readonly TrashGroup[];
  readonly total_bytes: number;
  readonly refused: readonly TrashRefusal[];
}

export interface TrashPeer {
  readonly peer_id: string;
  readonly name: string;
  readonly online: boolean;
}

export interface ProjectDeletePreview extends TrashLeftovers {
  /** "Trash", "Recycle Bin", or "trash folder" (the one in the service's home). */
  readonly bin: string;
  /** The linked computers the delete tells to trash their copy. */
  readonly peers: readonly TrashPeer[];
}

export interface DeletedProjectsPreview {
  readonly projects: readonly TrashLeftovers[];
  readonly total_bytes: number;
  readonly bin: string;
  /** Linked computers asked about a deleted project that haven't confirmed. */
  readonly linked: readonly {
    readonly project_name: string;
    readonly peer_name: string;
    readonly state: "pending" | "unconfirmed" | (string & Record<never, never>);
  }[];
}

export type ProjectTrashRequestBody =
  | { readonly type: "project_delete_preview"; readonly project_id: string }
  | { readonly type: "deleted_projects_preview" }
  | { readonly type: "deleted_projects_cleanup" };

export type ProjectTrashReply =
  | { readonly type: "project_delete_preview"; readonly preview: ProjectDeletePreview }
  | { readonly type: "deleted_projects_preview"; readonly preview: DeletedProjectsPreview }
  | {
      readonly type: "deleted_projects_cleaned";
      readonly moved_bytes: number;
      readonly summary: string;
      readonly skipped: readonly TrashRefusal[];
    };

/** "the Trash", "the Recycle Bin", "the trash folder". */
function theBin(bin: string): string {
  return `the ${bin}`;
}

/**
 * "This moves 1.6 GB to the Trash: project files 380 MB, bot session history
 * 1.2 GB, git cache 3 MB."
 */
export function trashSentence(preview: ProjectDeletePreview): string {
  if (preview.groups.length === 0) {
    return "It left nothing else on disk.";
  }
  const parts = preview.groups.map((g) => `${g.label} ${sizeText(g.bytes)}`).join(", ");
  return `This moves ${sizeText(preview.total_bytes)} to ${theBin(preview.bin)}: ${parts}.`;
}

/** "Linked computers told to trash their copy: win, pc (offline, told when it's back)." */
export function peersSentence(peers: readonly TrashPeer[]): string | null {
  if (peers.length === 0) {
    return null;
  }
  const names = peers
    .map((p) => (p.online ? p.name : `${p.name} (offline, told when it's back)`))
    .join(", ");
  const whom = peers.length === 1 ? "Linked computer" : "Linked computers";
  return `${whom} told to trash ${peers.length === 1 ? "its" : "their"} copy: ${names}.`;
}

/** "2 items are left where they are: …" for what may not be touched. */
export function refusedSentence(refused: readonly TrashRefusal[]): string | null {
  const [first] = refused;
  if (first === undefined) {
    return null;
  }
  const what = refused.length === 1 ? "1 item is" : `${refused.length} items are`;
  return `${what} left where ${refused.length === 1 ? "it is" : "they are"}: ${first.reason}.`;
}

/** The cleanup's confirmation: "This moves 1.4 GB to the Trash: Old 1.2 GB, Other 200 MB." */
export function deletedSentence(preview: DeletedProjectsPreview): string {
  const parts = preview.projects
    .map((p) => `${p.project_name} ${sizeText(p.total_bytes)}`)
    .join(", ");
  return `This moves ${sizeText(preview.total_bytes)} to ${theBin(preview.bin)}: ${parts}.`;
}

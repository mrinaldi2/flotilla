// A draft's changes in words (0.19.0): which of them the owner hasn't
// looked at, who made them, and each as a field diff, an added criterion,
// or an attached or linked document.

import type { DraftRevision } from "../../../protocol/gen/flotilla/board/v1/board_pb";

/** The owner's own changes: on the token or through a device. */
export function isOwners(actor: string): boolean {
  return actor === "user" || actor === "owner" || actor.startsWith("device:");
}

/** The bots' changes after the last one the owner has seen, oldest first. */
export function unseenChanges(revisions: readonly DraftRevision[], seen: bigint): DraftRevision[] {
  return revisions.filter((r) => r.id > seen && !isOwners(r.actor));
}

/** "3 changes by Lead since you looked"; "2 changes by Lead and Builder …". */
export function unseenLine(
  changes: readonly DraftRevision[],
  who: (actor: string) => string,
): string {
  const names = [...new Set(changes.map((c) => who(c.actor)))];
  const by =
    names.length <= 1
      ? (names[0] ?? "a bot")
      : `${names.slice(0, -1).join(", ")} and ${names.at(-1) ?? ""}`;
  return `${changes.length} ${changes.length === 1 ? "change" : "changes"} by ${by} since you looked`;
}

const FIELDS: Readonly<Record<string, string>> = {
  title: "Title",
  description: "Description",
  type: "Type",
  size: "Size",
  priority: "Priority",
  labels: "Labels",
  acceptance_criteria: "Acceptance criteria",
  attachment: "Attachment",
  link: "Link",
  parent: "Parent",
};

function lines(text: string | undefined): string[] {
  return (text ?? "")
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
}

function short(text: string): string {
  return text === "" ? "—" : text.length > 80 ? `${text.slice(0, 79)}…` : text;
}

export interface ChangeWords {
  /** "Title", "Acceptance criteria". */
  readonly field: string;
  /** "+" added, "−" removed, "~" changed: shown with words, never colour alone. */
  readonly mark: "+" | "−" | "~";
  /** What it says now, in one line. */
  readonly summary: string;
  /** For a text field: the before and after. */
  readonly before?: string;
  readonly after?: string;
}

/** An attachment or a link, added or removed. */
function attached(r: DraftRevision, field: string): ChangeWords {
  const what = r.to ?? r.from ?? "";
  const added = r.to !== undefined;
  const noun = r.field === "attachment" ? "Attached" : "Linked";
  return { field, mark: added ? "+" : "−", summary: added ? `${noun} ${what}` : `Removed ${what}` };
}

/** Criteria added and removed, one line each. */
function criteria(r: DraftRevision, field: string): ChangeWords {
  const before = lines(r.from);
  const after = lines(r.to);
  const added = after.filter((c) => !before.includes(c));
  const removed = before.filter((c) => !after.includes(c));
  const parts = [...added.map((c) => `+ ${c}`), ...removed.map((c) => `− ${c}`)];
  const mark = removed.length === 0 ? "+" : added.length === 0 ? "−" : "~";
  return { field, mark, summary: parts.length === 0 ? "Reordered" : parts.join("; ") };
}

/** One revision in words. */
export function changeWords(r: DraftRevision): ChangeWords {
  const field = FIELDS[r.field] ?? r.field;
  if (r.field === "attachment" || r.field === "link") {
    return attached(r, field);
  }
  if (r.field === "acceptance_criteria") {
    return criteria(r, field);
  }
  const before = r.from ?? "";
  const after = r.to ?? "";
  const summary = r.field === "description" ? "Rewritten" : `${short(before)} → ${short(after)}`;
  return { field, mark: "~", summary, before, after };
}

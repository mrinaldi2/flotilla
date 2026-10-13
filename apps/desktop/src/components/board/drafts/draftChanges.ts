// A draft's changes in words (0.19.0): which of them the owner hasn't
// looked at, who made them, and each as a field diff, an added criterion,
// or an attached or linked document.

import type { DraftRevision } from "../../../protocol/gen/hermes/board/v1/board_pb";

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

/** One revision in words. */
export function changeWords(r: DraftRevision): ChangeWords {
  const field = FIELDS[r.field] ?? r.field;
  if (r.field === "attachment" || r.field === "link") {
    const what = r.to ?? r.from ?? "";
    const added = r.to !== undefined;
    const noun = r.field === "attachment" ? "Attached" : "Linked";
    return {
      field,
      mark: added ? "+" : "−",
      summary: added ? `${noun} ${what}` : `Removed ${what}`,
    };
  }
  if (r.field === "acceptance_criteria") {
    const before = lines(r.from);
    const after = lines(r.to);
    const added = after.filter((c) => !before.includes(c));
    const removed = before.filter((c) => !after.includes(c));
    const parts = [...added.map((c) => `+ ${c}`), ...removed.map((c) => `− ${c}`)];
    return {
      field,
      mark: removed.length === 0 ? "+" : added.length === 0 ? "−" : "~",
      summary: parts.length === 0 ? "Reordered" : parts.join("; "),
    };
  }
  const before = r.from ?? "";
  const after = r.to ?? "";
  return {
    field,
    mark: "~",
    summary: r.field === "description" ? "Rewritten" : `${short(before)} → ${short(after)}`,
    before,
    after,
  };
}

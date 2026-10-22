// The board runs the SDLC (0.24.0, `board_flow_v1`): who holds the role a
// column assigns on entry, the roles nobody holds, and the chain a card
// follows, in words.

import type { Bot } from "../../../protocol/entities";
import type { BoardColumn, ProjectRole } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { ColumnKind, Role } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { roleLabel } from "../../bot/roleLabels";

/** What Board settings needs to show and fix each column's role. */
export interface ColumnFlow {
  readonly roles: readonly ProjectRole[];
  readonly bots: readonly Bot[];
  /** Creates a bot with the role; absent for a reader who can't. */
  readonly onCreateBot?: (role: Role) => void;
  /** The role whose bot is being created. */
  readonly creating?: Role | null;
}

/** "Held by": the holders' names, or why there are none. */
export interface Held {
  readonly text: string;
  /** The column assigns a role nobody holds. */
  readonly unheld: boolean;
}

/** "Architect · asleep" for one holder, "Unity Dev, RL" for several, "nobody yet" for none. */
export function heldBy(role: Role, flow: Pick<ColumnFlow, "roles" | "bots">): Held {
  if (role === Role.UNSPECIFIED) {
    return { text: "—", unheld: false };
  }
  const holders = flow.roles
    .filter((r) => r.role === role)
    .map((r) => flow.bots.find((b) => b.id === r.botId));
  if (holders.length === 0) {
    return { text: "nobody yet", unheld: true };
  }
  const [only] = holders;
  if (holders.length === 1 && only?.state === "asleep") {
    return { text: `${only.name} · asleep`, unheld: false };
  }
  return { text: holders.map((b) => b?.name ?? "a bot").join(", "), unheld: false };
}

/** A role some column assigns and nobody holds, with the columns that assign it. */
interface Unheld {
  readonly role: Role;
  readonly columns: readonly string[];
}

/** Each role a column assigns that nobody holds, in board order. */
export function unheldRoles(
  columns: readonly BoardColumn[],
  roles: readonly ProjectRole[],
): readonly Unheld[] {
  const held = new Set(roles.map((r) => r.role));
  const found = new Map<Role, string[]>();
  for (const column of columns) {
    const role = column.onEntry;
    if (role !== Role.UNSPECIFIED && !held.has(role)) {
      found.set(role, [...(found.get(role) ?? []), column.name]);
    }
  }
  return [...found].map(([role, names]) => ({ role, columns: names }));
}

/** "To do", "To do and Inbox", "To do, Inbox and Ready". */
export function namesText(names: readonly string[]): string {
  if (names.length <= 1) {
    return names.join("");
  }
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1) ?? ""}`;
}

/** A chip of the chain: "Research · Architect", or the closing "Done". */
interface Step {
  readonly key: string;
  readonly text: string;
  readonly done: boolean;
}

/** The columns that assign a role, in order, then where the card closes. */
export function flowSteps(columns: readonly BoardColumn[]): readonly Step[] {
  const assigning = columns.filter((c) => c.onEntry !== Role.UNSPECIFIED);
  if (assigning.length === 0) {
    return [];
  }
  const done = columns.find((c) => c.kind === ColumnKind.DONE);
  return [
    ...assigning.map((c) => ({
      key: c.key,
      text: `${c.name} · ${roleLabel(c.onEntry)}`,
      done: false,
    })),
    ...(done === undefined ? [] : [{ key: done.key, text: done.name, done: true }]),
  ];
}

/** What a failed review does, when a column assigns the Tester and one the Developer. */
export function sendBackLine(columns: readonly BoardColumn[]): string | null {
  const dev = columns.find((c) => c.onEntry === Role.DEV);
  const tests = columns.some((c) => c.onEntry === Role.TESTER);
  return dev === undefined || !tests
    ? null
    : `A failed review moves it back to ${dev.name} and re-assigns the Developer, with the Tester's note.`;
}

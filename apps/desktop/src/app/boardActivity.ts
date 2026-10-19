// Board activity as toasts (0.19.0): what bots, the owner on another device
// and the daemon do on the boards, turned into a short stack that drops from
// the top. Notices about one actor and one card within `GROUP_WINDOW_MS`
// fold into one toast, so a bot that moves a card and finishes its turn says
// so once; at most `MAX_ACTIVITY_TOASTS` show, newest on top.

import { BoardActivityKind as Kind } from "../protocol/gen/flotilla/home/v1/home_pb";
import type { BoardActivity } from "../protocol/gen/flotilla/home/v1/home_pb";
import type { BotState } from "../protocol/entities";
import type { BoardActivityKind, BoardActivityPrefs } from "../prefs";

export const GROUP_WINDOW_MS = 10_000;
export const ACTIVITY_TTL_MS = 8_000;
export const MAX_ACTIVITY_TOASTS = 3;
/** Lines one toast keeps; older ones scroll off. */
const MAX_LINES = 4;

/** One thing that happened: a glyph and a word, never colour alone. */
export interface ActivityLine {
  readonly glyph: string;
  readonly word: string;
  readonly detail: string;
}

export interface ActivityNotice {
  readonly pref: BoardActivityKind;
  /** As history stores it (`bot:<id>`, `user`, `device:<id>`, `daemon`). */
  readonly actor: string;
  readonly actorLabel: string;
  readonly projectId: string;
  readonly itemId: string;
  readonly itemTitle: string;
  readonly line: ActivityLine;
}

export interface ActivityToast {
  readonly id: number;
  readonly key: string;
  readonly actorLabel: string;
  readonly projectId: string;
  readonly itemId: string;
  readonly itemTitle: string;
  readonly lines: readonly ShownLine[];
  readonly updatedAt: number;
}

/** A line on a toast, keyed for rendering. */
interface ShownLine extends ActivityLine {
  readonly id: number;
}

export interface ActivityStack {
  readonly toasts: readonly ActivityToast[];
  readonly nextId: number;
}

export const EMPTY_STACK: ActivityStack = { toasts: [], nextId: 1 };

function keyOf(notice: ActivityNotice): string {
  return [notice.actor, notice.projectId, notice.itemId].join("\u0000");
}

/** Adds a notice: folded into its actor's toast for the card, or a new one on top. */
export function addNotice(
  stack: ActivityStack,
  notice: ActivityNotice,
  now: number,
): ActivityStack {
  const key = keyOf(notice);
  const same = stack.toasts.find((t) => t.key === key && now - t.updatedAt <= GROUP_WINDOW_MS);
  const others = stack.toasts.filter((t) => t !== same);
  // One counter numbers toasts and their lines alike.
  const line: ShownLine = { ...notice.line, id: stack.nextId };
  const toast: ActivityToast =
    same === undefined
      ? {
          id: stack.nextId,
          key,
          actorLabel: notice.actorLabel,
          projectId: notice.projectId,
          itemId: notice.itemId,
          itemTitle: notice.itemTitle,
          lines: [line],
          updatedAt: now,
        }
      : {
          ...same,
          itemTitle: notice.itemTitle === "" ? same.itemTitle : notice.itemTitle,
          lines: [...same.lines, line].slice(-MAX_LINES),
          updatedAt: now,
        };
  return {
    toasts: [toast, ...others].slice(0, MAX_ACTIVITY_TOASTS),
    nextId: stack.nextId + 1,
  };
}

/** Drops the toasts nothing has updated for `ACTIVITY_TTL_MS`. */
export function expireStack(stack: ActivityStack, now: number): ActivityStack {
  const toasts = stack.toasts.filter((t) => now - t.updatedAt < ACTIVITY_TTL_MS);
  return toasts.length === stack.toasts.length ? stack : { ...stack, toasts };
}

export function dismissToast(stack: ActivityStack, id: number): ActivityStack {
  return { ...stack, toasts: stack.toasts.filter((t) => t.id !== id) };
}

export function allowed(notice: ActivityNotice, prefs: BoardActivityPrefs): boolean {
  return prefs[notice.pref];
}

/** Who did it, as a toast names them. */
export function actorLabel(
  actor: string,
  actorName: string,
  botName: (botId: string) => string | undefined,
): string {
  if (actor.startsWith("bot:")) {
    return botName(actor.slice(4)) ?? (actorName === "" ? "A bot" : actorName);
  }
  if (actor === "user") {
    return "You";
  }
  if (actor.startsWith("device:")) {
    return actorName === "" ? "You, on another device" : `You, on ${actorName}`;
  }
  return "Automatic";
}

function joined(...parts: readonly string[]): string {
  return parts.filter((p) => p !== "").join(" · ");
}

function lineFor(a: BoardActivity): { pref: BoardActivityKind; line: ActivityLine } | null {
  switch (a.kind) {
    case Kind.MOVED:
      return {
        pref: "moved",
        line: { glyph: "→", word: "Moved", detail: joined(`${a.fromName} → ${a.toName}`, a.note) },
      };
    case Kind.ASSIGNED:
      return a.actor === `bot:${a.assignee}`
        ? { pref: "assigned", line: { glyph: "+", word: "Picked up", detail: "" } }
        : {
            pref: "assigned",
            line: { glyph: "+", word: "Assigned", detail: `to ${a.assigneeName}` },
          };
    case Kind.UNASSIGNED:
      return { pref: "assigned", line: { glyph: "−", word: "Unassigned", detail: "" } };
    case Kind.MOVE_REFUSED:
      return {
        pref: "refused",
        line: {
          glyph: "⊘",
          word: "Couldn't move",
          detail: joined(`to ${a.toName}`, a.unmet.join(" ")),
        },
      };
    case Kind.DRAFT_UPDATED:
      return { pref: "drafts", line: { glyph: "✎", word: "Updated the draft", detail: a.note } };
    // The board keeper: the daemon asked the assignee, or raised a row.
    case Kind.CHECKED:
      return a.assignee === ""
        ? { pref: "checks", line: { glyph: "◷", word: "Needs you", detail: a.note } }
        : {
            pref: "checks",
            line: { glyph: "◷", word: `Asked ${a.assigneeName}`, detail: a.note },
          };
    default:
      return null;
  }
}

/**
 * The notice for a `board_activity` push, or none: the owner's own action on
 * this app already showed, and a kind this app doesn't know says nothing.
 */
export function noticeFromActivity(
  a: BoardActivity,
  botName: (botId: string) => string | undefined,
): ActivityNotice | null {
  if (a.fromThisClient) {
    return null;
  }
  const line = lineFor(a);
  if (line === null) {
    return null;
  }
  return {
    ...line,
    actor: a.actor,
    actorLabel: actorLabel(a.actor, a.actorName, botName),
    projectId: a.projectId,
    itemId: a.itemId,
    itemTitle: a.itemTitle,
  };
}

const IN_TURN: ReadonlySet<BotState> = new Set<BotState>([
  "working",
  "waiting_for_user",
  "waiting_for_approval",
]);

export interface BotStatePush {
  readonly bot_id: string;
  readonly state: BotState;
  readonly item_id?: string;
  readonly item_title?: string;
  /** "answer": the owner's answer to the bot's question started the turn (0.22.0). */
  readonly cause?: string;
}

export interface TurnBot {
  readonly name: string;
  readonly projectId: string;
}

/**
 * A turn starting or finishing on a card, from a `bot_state` push. `open`
 * holds the card of each bot's running turn, so an approval wait inside a
 * turn isn't a new start. A turn about no card, a chat, says nothing. A
 * turn the owner's answer started says it got the answer.
 */
export function noticeFromBotState(
  push: BotStatePush,
  open: Map<string, string>,
  bot: TurnBot | undefined,
): ActivityNotice | null {
  const running = open.get(push.bot_id);
  const inTurn = IN_TURN.has(push.state);
  // A turn about no card at its start may end on one the bot changed.
  const ended = push.item_id ?? running;
  let line: ActivityLine;
  let itemId: string;
  if (inTurn && running === undefined && push.item_id !== undefined) {
    open.set(push.bot_id, push.item_id);
    line =
      push.cause === "answer"
        ? { glyph: "●", word: "Got your answer", detail: "working on it" }
        : { glyph: "●", word: "Started", detail: "working on it" };
    itemId = push.item_id;
  } else if (!inTurn && ended !== undefined) {
    open.delete(push.bot_id);
    line = { glyph: "✓", word: "Finished", detail: "a turn" };
    itemId = ended;
  } else {
    return null;
  }
  if (bot === undefined) {
    return null;
  }
  return {
    pref: "turns",
    actor: `bot:${push.bot_id}`,
    actorLabel: bot.name,
    projectId: bot.projectId,
    itemId,
    itemTitle: push.item_title ?? "",
    line,
  };
}

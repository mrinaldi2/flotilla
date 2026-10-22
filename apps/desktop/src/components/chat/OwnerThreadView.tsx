// A bot's thread with the owner (H-132 D6): the one conversation the owner
// has with it (H-192), shown on the bot's own Chat tab and in a draft's
// Discuss, with one read marker.

import { useEffect, useRef } from "react";
import type { ReactElement, ReactNode } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { ThreadMessage } from "../../protocol/gen/flotilla/home/v1/home_pb";
import { unverifiedLabel } from "../../protocol/home";
import LinkedText from "../cards/LinkedText";
import { clock } from "../home/homeText";
import { useOwnerThread } from "../home/useOwnerThreads";

/**
 * One message: the owner's on the right, the bot's on the left. The owner's
 * chat a linked computer sent unverified is never shown as theirs (H-306).
 */
function Bubble(props: { readonly message: ThreadMessage; readonly now: number }): ReactElement {
  const { message, now } = props;
  const mine = message.fromOwner;
  const unverified = message.unverifiedFrom !== "";
  const label =
    message.asks && message.open
      ? "Asks you"
      : mine
        ? "You"
        : unverified
          ? unverifiedLabel(message.unverifiedFrom)
          : "";
  const kind = mine ? " mc-bubble-me" : unverified ? " mc-bubble-unverified" : "";
  return (
    <div className={`mc-bubble${kind}`}>
      <span className="mc-bubble-src">
        {label}
        {message.at === undefined ? "" : `${label ? " · " : ""}${clock(message.at, now)}`}
      </span>
      <span className="mc-bubble-text">
        <LinkedText text={message.text} />
      </span>
    </div>
  );
}

/** Marks a thread read up to its newest message once the owner has it open. */
function useMarkRead(
  client: DaemonApi,
  botId: string,
  newest: bigint | undefined,
  lastRead: bigint | undefined,
): void {
  useEffect(() => {
    if (newest === undefined || (lastRead !== undefined && lastRead >= newest)) {
      return;
    }
    client
      .request(
        { type: "owner_thread_read", bot_id: botId, up_to_num: Number(newest) },
        "owner_thread_marked",
      )
      .catch(() => {
        // Read state is a convenience; the badge clears on the next read.
      });
  }, [client, botId, newest, lastRead]);
}

/** One bot's thread with the owner, oldest first, scrolled to the newest. */
export default function OwnerThreadView(props: {
  readonly client: DaemonApi;
  readonly botId: string;
  readonly connected: boolean;
  readonly now: number;
  /** Shown after the messages, given them: e.g. where an answer may be. */
  readonly after?: (messages: readonly ThreadMessage[]) => ReactNode;
}): ReactElement {
  const page = useOwnerThread(props.client, props.botId, props.connected);
  const end = useRef<HTMLDivElement | null>(null);
  const messages = page?.messages ?? [];
  useMarkRead(props.client, props.botId, messages.at(-1)?.num, page?.lastReadNum);
  const newest = messages.at(-1)?.id;
  useEffect(() => {
    // Each new message scrolls the thread to it.
    if (newest !== undefined) {
      end.current?.scrollIntoView?.({ block: "end" });
    }
  }, [newest]);
  return (
    <div className="mc-messages" aria-label="Messages">
      {messages.length === 0 ? (
        <p className="dash-empty">No messages yet. What you send shows here, and the answer too.</p>
      ) : (
        messages.map((m) => <Bubble key={m.id} message={m} now={props.now} />)
      )}
      {props.after?.(messages)}
      <div ref={end} />
    </div>
  );
}

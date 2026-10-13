// "Discuss with a bot" (0.19.0): the bot's chat docked beside the drawer,
// pinned to the draft. Every message goes with the item id, so the bot
// reads the draft with it; its changes show as one compact row, and the
// drawer beside it refreshes live.

import { X } from "lucide-react";
import type { ReactElement } from "react";
import { nativeDictation } from "../../../dictation";
import type { DaemonApi } from "../../../protocol/api";
import type { Bot } from "../../../protocol/entities";
import ChatComposer from "../../chat/ChatComposer";
import OwnerThreadView from "../../mainchat/OwnerThreadView";

interface DiscussPanelProps {
  readonly client: DaemonApi;
  readonly bot: Bot;
  readonly itemId: string;
  readonly connected: boolean;
  /** The bot's changes the owner hasn't reviewed, for the "↻ updated the draft" row. */
  readonly changes: number;
  readonly onReview: () => void;
  readonly onClose: () => void;
  readonly now: number;
}

export default function DiscussPanel(props: DiscussPanelProps): ReactElement {
  const { client, bot, itemId } = props;
  const send = async (text: string): Promise<void> => {
    // A failed send throws, so the composer keeps the text.
    await client.request(
      { type: "send_user_message", to_bot_id: bot.id, body: text, item_id: itemId },
      "message",
    );
  };
  return (
    <aside className="discuss-panel" aria-label={`${bot.name} about ${itemId}`}>
      <header className="drawer-head">
        <span className="drawer-type">
          <b>{bot.name}</b> · about <span className="mono">{itemId}</span>
        </span>
        <button
          type="button"
          className="drawer-close"
          aria-label={`Close the chat with ${bot.name}`}
          onClick={props.onClose}
        >
          <X size={16} aria-hidden="true" />
        </button>
      </header>
      <div className="discuss-thread">
        <OwnerThreadView
          client={client}
          botId={bot.id}
          connected={props.connected}
          now={props.now}
        />
      </div>
      {props.changes === 0 ? null : (
        <p className="discuss-updated" role="status">
          <span aria-hidden="true">↻ </span>
          {`${bot.name} updated the draft · ${props.changes} ${props.changes === 1 ? "change" : "changes"}`}
          <button type="button" className="chat-link" onClick={props.onReview}>
            Review
          </button>
        </p>
      )}
      <ChatComposer
        disabledReason={props.connected ? null : "Waiting for the Hermes service"}
        placeholder={`Message ${bot.name} about ${itemId}…`}
        onSend={send}
        dictation={nativeDictation}
      />
    </aside>
  );
}

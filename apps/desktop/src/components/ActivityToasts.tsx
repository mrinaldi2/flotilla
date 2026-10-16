import { useEffect, useRef } from "react";
import type { KeyboardEvent, ReactElement } from "react";
import type { ActivityToast } from "../app/boardActivity";

interface ActivityToastsProps {
  readonly toasts: readonly ActivityToast[];
  readonly onOpen: (toast: ActivityToast) => void;
  readonly onDismiss: (id: number) => void;
  /** Hovering or focusing the stack holds it up; leaving lets it go. */
  readonly onHold: (held: boolean) => void;
}

function cardName(toast: ActivityToast): string {
  return toast.itemTitle === "" ? toast.itemId : `${toast.itemId} “${toast.itemTitle}”`;
}

/** What a screen reader hears for a toast: who, which card, what happened. */
function spoken(toast: ActivityToast): string {
  const lines = toast.lines.map((l) => [l.word, l.detail].filter((p) => p !== "").join(" "));
  return `${toast.actorLabel}, ${cardName(toast)}: ${lines.join("; ")}`;
}

/**
 * Board activity, dropping from the top of the window over every view
 * (0.19.0). Each toast opens its card; Escape or × dismisses it. The stack
 * is a polite live region, so a screen reader hears each one once.
 */
export default function ActivityToasts({
  toasts,
  onOpen,
  onDismiss,
  onHold,
}: ActivityToastsProps): ReactElement {
  const stack = useRef<HTMLElement>(null);
  // Reading or tabbing through the stack holds it up. Listened to directly:
  // holding is the stack's behaviour, not something the owner operates.
  useEffect(() => {
    const el = stack.current;
    if (el === null) {
      return undefined;
    }
    const hold = (): void => {
      onHold(true);
    };
    const release = (): void => {
      onHold(false);
    };
    const blur = (event: FocusEvent): void => {
      if (!(event.relatedTarget instanceof Node && el.contains(event.relatedTarget))) {
        onHold(false);
      }
    };
    el.addEventListener("mouseenter", hold);
    el.addEventListener("mouseleave", release);
    el.addEventListener("focusin", hold);
    el.addEventListener("focusout", blur);
    return () => {
      el.removeEventListener("mouseenter", hold);
      el.removeEventListener("mouseleave", release);
      el.removeEventListener("focusin", hold);
      el.removeEventListener("focusout", blur);
    };
  }, [onHold]);
  const escape = (event: KeyboardEvent<HTMLButtonElement>, id: number): void => {
    if (event.key === "Escape") {
      event.stopPropagation();
      onDismiss(id);
    }
  };
  return (
    <section
      ref={stack}
      className="activity-toasts"
      aria-label="Board activity"
      aria-live="polite"
      aria-relevant="additions text"
    >
      {toasts.map((toast) => (
        <div key={toast.id} className="activity-toast">
          <button
            type="button"
            className="activity-toast-open"
            aria-label={`${spoken(toast)}. Open the card`}
            onClick={() => {
              onOpen(toast);
            }}
            onKeyDown={(event) => {
              escape(event, toast.id);
            }}
          >
            <span className="activity-toast-head">
              <span className="activity-toast-actor">{toast.actorLabel}</span>
              <span className="activity-toast-card">{cardName(toast)}</span>
            </span>
            {toast.lines.map((line) => (
              <span key={line.id} className="activity-toast-line">
                <span className="activity-toast-glyph" aria-hidden="true">
                  {line.glyph}
                </span>
                <span className="activity-toast-word">{line.word}</span>
                {line.detail === "" ? null : (
                  <span className="activity-toast-detail">{line.detail}</span>
                )}
              </span>
            ))}
          </button>
          <button
            type="button"
            className="toast-close"
            aria-label={`Dismiss: ${toast.actorLabel}, ${toast.itemId}`}
            onClick={() => {
              onDismiss(toast.id);
            }}
            onKeyDown={(event) => {
              escape(event, toast.id);
            }}
          >
            ×
          </button>
        </div>
      ))}
    </section>
  );
}

// A button that opens a menu below it (WAI-ARIA menu button): ↓ or Enter
// opens it on the first item, ↑/↓/Home/End move, Enter or a click chooses,
// Esc or a press outside closes it and returns focus to the button.

import { ChevronDown } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import type { KeyboardEvent, ReactElement, ReactNode } from "react";

export interface MenuEntry {
  readonly key: string;
  readonly label: ReactNode;
  /** Read by screen readers when `label` isn't plain words. */
  readonly ariaLabel?: string;
  /** A second line, dimmed. */
  readonly note?: string;
  /** The current choice (menuitemradio). */
  readonly checked?: boolean;
  readonly disabled?: boolean;
  /** A rule above this entry. */
  readonly separated?: boolean;
  readonly onChoose: () => void;
}

interface MenuButtonProps {
  readonly label: ReactNode;
  /** The button's accessible name, when `label` isn't enough. */
  readonly ariaLabel?: string;
  readonly menuLabel: string;
  readonly entries: readonly MenuEntry[];
  readonly className?: string;
  readonly disabled?: boolean;
  /** Opens the menu at once (stories). */
  readonly initiallyOpen?: boolean;
  /** Hangs the menu from the button's right edge. */
  readonly alignEnd?: boolean;
}

/** Where ↑, ↓, Home and End move focus among `count` items from `at`; null for other keys. */
function step(key: string, at: number, count: number): number | null {
  switch (key) {
    case "ArrowDown":
      return (at + 1) % count;
    case "ArrowUp":
      return (at - 1 + count) % count;
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

export default function MenuButton(props: MenuButtonProps): ReactElement {
  const { entries } = props;
  const [open, setOpen] = useState(props.initiallyOpen === true);
  const root = useRef<HTMLDivElement | null>(null);
  const button = useRef<HTMLButtonElement | null>(null);
  const list = useRef<HTMLDivElement | null>(null);
  const id = useId();

  const items = (): HTMLElement[] => [
    ...(list.current?.querySelectorAll<HTMLElement>("[role^=menuitem]:not([disabled])") ?? []),
  ];
  const close = (focusButton: boolean): void => {
    setOpen(false);
    if (focusButton) {
      button.current?.focus();
    }
  };

  useEffect(() => {
    if (!open) {
      return undefined;
    }
    const onPointer = (event: MouseEvent): void => {
      if (event.target instanceof Node && root.current?.contains(event.target) !== true) {
        setOpen(false);
      }
    };
    window.addEventListener("mousedown", onPointer);
    return () => window.removeEventListener("mousedown", onPointer);
  }, [open]);

  useEffect(() => {
    if (open && props.initiallyOpen !== true) {
      items()[0]?.focus();
    }
    // Focus moves in only when the owner opened it, not in a story.
    // oxlint-disable-next-line react/exhaustive-effect-dependencies
  }, [open, props.initiallyOpen]);

  const onMenuKey = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close(true);
      return;
    }
    if (event.key === "Tab") {
      close(false);
      return;
    }
    const all = items();
    const at = all.findIndex((el) => el === document.activeElement);
    const next = step(event.key, at, all.length);
    if (next !== null) {
      event.preventDefault();
      all[next]?.focus();
    }
  };

  return (
    <div className={`menu-button ${props.className ?? ""}`} ref={root}>
      <button
        ref={button}
        type="button"
        className="board-filter-button menu-button-trigger"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        aria-label={props.ariaLabel}
        disabled={props.disabled}
        onClick={() => setOpen((value) => !value)}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" && !open) {
            event.preventDefault();
            setOpen(true);
          }
        }}
      >
        {props.label}
        <ChevronDown size={12} aria-hidden="true" />
      </button>
      {open ? (
        <div
          id={id}
          ref={list}
          role="menu"
          tabIndex={-1}
          aria-label={props.menuLabel}
          className={`menu-popup${props.alignEnd === true ? " menu-popup-end" : ""}`}
          onKeyDown={onMenuKey}
        >
          {entries.map((entry) => (
            <button
              key={entry.key}
              type="button"
              role={entry.checked === undefined ? "menuitem" : "menuitemradio"}
              aria-checked={entry.checked}
              aria-label={entry.ariaLabel}
              tabIndex={-1}
              disabled={entry.disabled}
              className={`menu-item${entry.separated === true ? " menu-item-separated" : ""}`}
              onClick={() => {
                close(true);
                entry.onChoose();
              }}
            >
              <span className="menu-item-mark" aria-hidden="true">
                {entry.checked === true ? "✓" : ""}
              </span>
              <span className="menu-item-text">
                <span>{entry.label}</span>
                {entry.note === undefined ? null : (
                  <span className="menu-item-note">{entry.note}</span>
                )}
              </span>
            </button>
          ))}
        </div>
      ) : null}
    </div>
  );
}

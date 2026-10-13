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
    const all = items();
    const at = all.findIndex((el) => el === document.activeElement);
    const next =
      event.key === "ArrowDown"
        ? (at + 1) % all.length
        : event.key === "ArrowUp"
          ? (at - 1 + all.length) % all.length
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? all.length - 1
              : null;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close(true);
    } else if (event.key === "Tab") {
      close(false);
    } else if (next !== null) {
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

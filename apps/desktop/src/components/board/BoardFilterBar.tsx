import { ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ReactElement } from "react";
import type { Bot } from "../../protocol/entities";
import BotAvatar from "../BotAvatar";
import type { BoardFilters } from "./filters";
import { isFiltering, NO_FILTERS, toggled } from "./filters";
import type { LabelGroupView } from "./labelGroups";
import { groupLabel } from "./labelGroups";
import { FILTER_TYPES, typeLabel } from "./labels";

/** A bot the Bot filter offers: a project bot, or an assignee the sidebar doesn't know. */
export interface BotOption {
  readonly id: string;
  readonly bot: Bot | undefined;
}

interface BoardFilterBarProps {
  readonly filters: BoardFilters;
  readonly onChange: (filters: BoardFilters) => void;
  readonly botOptions: readonly BotOption[];
  /** The board's label groups, each a row of chips. */
  readonly labelGroups: readonly LabelGroupView[];
  readonly shown: number;
  readonly total: number;
  /** Names of the hidden columns ("Cancelled"), offered behind a toggle. */
  readonly hiddenNames: readonly string[];
  readonly showHidden: boolean;
  readonly onShowHidden: (show: boolean) => void;
}

function BotMenu({
  options,
  selected,
  onToggle,
}: {
  readonly options: readonly BotOption[];
  readonly selected: readonly string[];
  readonly onToggle: (id: string) => void;
}): ReactElement {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const buttonRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (!open) {
      return undefined;
    }
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        setOpen(false);
        buttonRef.current?.focus();
      }
    };
    const onPointer = (event: MouseEvent): void => {
      if (event.target instanceof Node && rootRef.current?.contains(event.target) !== true) {
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("mousedown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("mousedown", onPointer);
    };
  }, [open]);

  return (
    <div className="board-filter-menu" ref={rootRef}>
      <button
        ref={buttonRef}
        type="button"
        className={`board-filter-button${selected.length > 0 ? " board-filter-on" : ""}`}
        aria-expanded={open}
        aria-haspopup="true"
        onClick={() => {
          setOpen((value) => !value);
        }}
      >
        {selected.length > 0 ? `Bot · ${selected.length}` : "Bot"}
        <ChevronDown size={12} aria-hidden="true" />
      </button>
      {open ? (
        <fieldset className="board-filter-popup">
          <legend className="visually-hidden">Show items assigned to</legend>
          {options.length === 0 ? <p className="board-popover-note">No bots yet.</p> : null}
          {options.map((option) => {
            const name = option.bot?.name ?? option.id;
            return (
              <label key={option.id} className="board-filter-option">
                <input
                  type="checkbox"
                  checked={selected.includes(option.id)}
                  onChange={() => {
                    onToggle(option.id);
                  }}
                />
                <BotAvatar avatar={option.bot?.avatar ?? ""} name={name} id={option.id} size="sm" />
                <span>{name}</span>
              </label>
            );
          })}
        </fieldset>
      ) : null}
    </div>
  );
}

function Chip({
  label,
  pressed,
  onToggle,
}: {
  readonly label: string;
  readonly pressed: boolean;
  readonly onToggle: () => void;
}): ReactElement {
  return (
    <button
      type="button"
      className={`board-filter-chip${pressed ? " board-filter-on" : ""}`}
      aria-pressed={pressed}
      onClick={onToggle}
    >
      {label}
    </button>
  );
}

/** The one-row filter bar above the board. */
export default function BoardFilterBar(props: BoardFilterBarProps): ReactElement {
  const { filters, onChange } = props;
  return (
    <div className="board-filters" role="toolbar" aria-label="Board filters">
      <BotMenu
        options={props.botOptions}
        selected={filters.bots}
        onToggle={(id) => {
          onChange({ ...filters, bots: toggled(filters.bots, id) });
        }}
      />
      {props.labelGroups.map((group) =>
        group.values.length === 0 ? null : (
          <div
            key={group.name}
            className="board-filter-group"
            role="group"
            aria-label={`Label group ${group.name}`}
          >
            <span className="board-filter-group-name">{group.name}</span>
            {group.values.map((value) => {
              const label = groupLabel(group.name, value);
              return (
                <Chip
                  key={label}
                  label={value}
                  pressed={filters.labels.includes(label)}
                  onToggle={() => {
                    onChange({ ...filters, labels: toggled(filters.labels, label) });
                  }}
                />
              );
            })}
          </div>
        ),
      )}
      <div className="board-filter-group" role="group" aria-label="Type">
        {FILTER_TYPES.map((type) => {
          const label = typeLabel(type);
          return (
            <Chip
              key={type}
              label={`${label.glyph} ${label.word}`}
              pressed={filters.types.includes(type)}
              onToggle={() => {
                onChange({ ...filters, types: toggled(filters.types, type) });
              }}
            />
          );
        })}
      </div>
      {props.hiddenNames.length === 0 ? null : (
        <label className="board-filter-toggle">
          <input
            type="checkbox"
            checked={props.showHidden}
            onChange={(event) => {
              props.onShowHidden(event.target.checked);
            }}
          />
          {`Show ${props.hiddenNames.join(", ").toLowerCase()}`}
        </label>
      )}
      {isFiltering(filters) ? (
        <span className="board-filter-summary" role="status">
          {`Showing ${props.shown} of ${props.total}`}
          <span aria-hidden="true"> · </span>
          <button
            type="button"
            className="board-filter-clear"
            onClick={() => {
              onChange(NO_FILTERS);
            }}
          >
            Clear
          </button>
        </span>
      ) : null}
    </div>
  );
}

// A column's entry policy as a checklist (0.19.0): who may move items in,
// which columns they may come from, and the entry rules with their
// parameters. Saved as one change, the lists replacing the column's.

import { useState } from "react";
import type { ReactElement } from "react";
import type { BoardColumn } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import {
  defaultArg,
  MOVERS,
  ROLES,
  RULES,
  rolesOf,
  splitToken,
  toggledRule,
  withRoles,
} from "./rules";
import type { RuleSpec } from "./rules";

export interface Policy {
  readonly movers: readonly string[];
  readonly rules: readonly string[];
  readonly allowedFrom: readonly string[];
}

interface ColumnRulesProps {
  readonly column: BoardColumn;
  /** The board's other columns, for "comes from". */
  readonly others: readonly BoardColumn[];
  readonly disabled: boolean;
  readonly onSave: (policy: Policy) => void;
}

function toggled(list: readonly string[], value: string, on: boolean): string[] {
  const rest = list.filter((v) => v !== value);
  return on ? [...rest, value] : rest;
}

/** One rule of the library: its checkbox and, once on, its parameter. */
function RuleRow(props: {
  readonly spec: RuleSpec;
  /** Undefined while off; null for a rule without a parameter. */
  readonly arg: string | null | undefined;
  readonly label: string;
  readonly onToggle: (on: boolean) => void;
  readonly onArg: (value: string) => void;
}): ReactElement {
  const { spec, arg } = props;
  const on = arg !== undefined;
  const param = on ? spec.param : { kind: "none" as const };
  return (
    <div className="column-rules-rule">
      <label className="column-rules-option">
        <input type="checkbox" checked={on} onChange={(e) => props.onToggle(e.target.checked)} />
        {spec.word}
      </label>
      {param.kind === "choice" ? (
        <select
          aria-label={props.label}
          value={arg ?? ""}
          onChange={(e) => props.onArg(e.target.value)}
        >
          {param.options.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      ) : null}
      {param.kind === "text" ? (
        <input
          type="text"
          aria-label={props.label}
          placeholder={param.hint}
          value={arg ?? ""}
          onChange={(e) => props.onArg(e.target.value)}
        />
      ) : null}
    </div>
  );
}

export default function ColumnRules(props: ColumnRulesProps): ReactElement {
  const { column, disabled } = props;
  const [movers, setMovers] = useState<readonly string[]>(column.movers);
  const [rules, setRules] = useState<readonly string[]>(column.rules);
  const [from, setFrom] = useState<readonly string[]>(column.allowedFrom);
  const roles = rolesOf(movers);
  const argOf = (head: string): string | null | undefined =>
    rules.map(splitToken).find((t) => t.head === head)?.arg;
  const changed =
    movers.join("|") !== column.movers.join("|") ||
    rules.join("|") !== column.rules.join("|") ||
    from.join("|") !== column.allowedFrom.join("|");
  const label = (what: string): string => `${column.name}: ${what}`;

  return (
    <div className="column-rules">
      <fieldset className="column-rules-group" disabled={disabled}>
        <legend>Who moves items in</legend>
        {MOVERS.map((m) => (
          <label key={m.token} className="column-rules-option">
            <input
              type="checkbox"
              checked={movers.includes(m.token)}
              onChange={(e) => setMovers(toggled(movers, m.token, e.target.checked))}
            />
            {m.word}
          </label>
        ))}
        <span className="column-rules-roles" role="group" aria-label={label("roles")}>
          Roles:
          {ROLES.map((role) => (
            <label key={role} className="column-rules-option">
              <input
                type="checkbox"
                checked={roles.includes(role)}
                onChange={(e) =>
                  setMovers(withRoles(movers, toggled(roles, role, e.target.checked)))
                }
              />
              {role}
            </label>
          ))}
        </span>
        <p className="column-rules-hint">None ticked: any member of the project.</p>
      </fieldset>
      <fieldset className="column-rules-group" disabled={disabled}>
        <legend>Comes only from</legend>
        {props.others.map((other) => (
          <label key={other.key} className="column-rules-option">
            <input
              type="checkbox"
              checked={from.includes(other.key)}
              onChange={(e) => setFrom(toggled(from, other.key, e.target.checked))}
            />
            {other.name}
          </label>
        ))}
        <p className="column-rules-hint">None ticked: any column.</p>
      </fieldset>
      <fieldset className="column-rules-group" disabled={disabled}>
        <legend>Entry rules</legend>
        {RULES.map((spec) => (
          <RuleRow
            key={spec.head}
            spec={spec}
            arg={argOf(spec.head)}
            label={label(spec.word)}
            onToggle={(on) => setRules(toggledRule(rules, spec.head, on, defaultArg(spec)))}
            onArg={(value) => setRules(toggledRule(rules, spec.head, true, value))}
          />
        ))}
      </fieldset>
      {disabled ? null : (
        <div className="column-rules-actions">
          <button
            type="button"
            className="btn btn-small"
            disabled={!changed}
            onClick={() => {
              setMovers(column.movers);
              setRules(column.rules);
              setFrom(column.allowedFrom);
            }}
          >
            Reset
          </button>
          <button
            type="button"
            className="btn btn-small btn-primary"
            disabled={!changed}
            onClick={() => props.onSave({ movers, rules, allowedFrom: from })}
          >
            Save rules
          </button>
        </div>
      )}
    </div>
  );
}

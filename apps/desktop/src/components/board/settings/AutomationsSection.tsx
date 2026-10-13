// Board settings › Automations (0.19.0): one row per fact the modules
// report, "When PR merged → move to Done". A fact with no column leaves the
// item where it is, and its card says so.

import type { ReactElement } from "react";
import type { Board, BoardColumn } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { FACTS } from "./rules";

interface AutomationsSectionProps {
  readonly board: Board;
  readonly columns: readonly BoardColumn[];
  readonly disabled: boolean;
  /** Maps `fact` to a column key, or unmaps it (undefined). */
  readonly onSet: (fact: string, columnKey: string | undefined) => void;
}

export default function AutomationsSection(props: AutomationsSectionProps): ReactElement {
  const { board, columns } = props;
  if (board.flow === "release_gated") {
    return (
      <section className="settings-section" aria-label="Automations">
        <p className="settings-note">
          <span aria-hidden="true">⚑</span> Release-gated: pull requests and releases move items
          through the built-in flow, by column. Automations are for custom boards.
        </p>
      </section>
    );
  }
  return (
    <section className="settings-section" aria-label="Automations">
      <table className="automations">
        <thead>
          <tr>
            <th scope="col">When</th>
            <th scope="col">Move to</th>
          </tr>
        </thead>
        <tbody>
          {FACTS.map((fact) => {
            const to = board.automations[fact.name];
            return (
              <tr key={fact.name}>
                <td>
                  {fact.word} <span className="mono settings-dim">{fact.name}</span>
                </td>
                <td>
                  <span aria-hidden="true">→ </span>
                  <select
                    aria-label={`When ${fact.word}, move to`}
                    value={to ?? ""}
                    disabled={props.disabled}
                    onChange={(e) =>
                      props.onSet(fact.name, e.target.value === "" ? undefined : e.target.value)
                    }
                  >
                    <option value="">— stays where it is</option>
                    {columns.map((column) => (
                      <option key={column.key} value={column.key}>
                        {column.name}
                      </option>
                    ))}
                  </select>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}

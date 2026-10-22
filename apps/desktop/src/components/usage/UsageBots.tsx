// The Usage page's "By bot" table (0.24.0, `usage_v1`): each bot's tokens,
// turns and share, for every project or one, whose shares then add up to it.

import { useState } from "react";
import type { ReactElement } from "react";
import type { Bot } from "../../protocol/entities";
import type { Usage } from "../../protocol/usage";
import { percentText, tokensText } from "./usageText";

const ALL = "";

/** The usage's name, else this computer's, else a removed bot's words. */
function botName(item: Usage["bots"][number], bots: readonly Bot[]): string {
  if (item.name !== "") {
    return item.name;
  }
  return bots.find((b) => b.id === item.bot_id)?.name ?? "A removed bot";
}

export default function UsageBots(props: {
  readonly usage: Usage;
  readonly bots: readonly Bot[];
}): ReactElement {
  const { usage, bots } = props;
  const [projectId, setProjectId] = useState(ALL);
  const project = usage.projects.find((p) => p.project_id === projectId);
  const shown =
    project === undefined ? usage.bots : usage.bots.filter((b) => b.project_id === projectId);
  // A project's bots share its tokens; every bot shares the total.
  const whole = project === undefined ? usage.total.tokens : project.totals.tokens;
  return (
    <section className="usage-card usage-by-bot" aria-labelledby="usage-bots-title">
      <div className="usage-card-head">
        <h2 className="usage-card-title" id="usage-bots-title">
          By bot
        </h2>
        <label className="usage-filter">
          Project
          <select
            value={project === undefined ? ALL : projectId}
            onChange={(e) => {
              setProjectId(e.target.value);
            }}
          >
            <option value={ALL}>All projects</option>
            {usage.projects.map((p) => (
              <option key={p.project_id} value={p.project_id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
      </div>
      {shown.length === 0 ? (
        <p className="usage-dim">No bot used any tokens in this period.</p>
      ) : (
        <table className="usage-table">
          <thead>
            <tr>
              <th scope="col">Bot</th>
              <th scope="col">Tokens</th>
              <th scope="col">Turns</th>
              <th scope="col">Share</th>
            </tr>
          </thead>
          <tbody>
            {shown.map((b) => (
              <tr key={`${b.daemon_id}:${b.bot_id}`}>
                <th scope="row">{botName(b, bots)}</th>
                <td>{tokensText(b.totals.tokens)}</td>
                <td className="usage-dim">{b.totals.turns.toLocaleString("en-US")}</td>
                <td className="usage-dim">
                  {percentText(whole > 0 ? b.totals.tokens / whole : 0)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </section>
  );
}

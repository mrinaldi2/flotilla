// The team on demand on the Team tab (0.23.0): how many bots run on this
// computer against the cap, and who waits for a slot, in the order they
// will start. An urgent bug (a P0 bug card) went to the front of card work.

import type { ReactElement } from "react";
import type { Bot, TeamStatus, TeamWake } from "../../protocol/entities";
import { runningText, WAKE_REASON_LABEL, waitedText } from "../../protocol/team";

interface TeamQueueProps {
  readonly team: TeamStatus;
  /** Every bot this app knows, for the names of bots from other projects too. */
  readonly bots: readonly Bot[];
  readonly now: number;
}

/** "Card T-12 · P1", "Lead", "You woke it". */
function whyText(wake: TeamWake): string {
  const reason = WAKE_REASON_LABEL[wake.reason] ?? wake.reason;
  const card = wake.item_id === undefined ? "" : ` ${wake.item_id}`;
  const priority = wake.priority === undefined ? "" : ` · ${wake.priority}`;
  return `${reason}${card}${priority}`;
}

function QueueRow(props: {
  readonly wake: TeamWake;
  readonly name: string;
  readonly now: number;
}): ReactElement {
  const { wake } = props;
  return (
    <li className="team-queue-row">
      <span className="team-queue-name">{props.name}</span>
      {wake.urgent ? <span className="team-badge team-urgent">Urgent bug</span> : null}
      <span className="team-queue-why">{whyText(wake)}</span>
      <span className="team-queue-since" title={wake.queued_at}>
        {waitedText(wake.queued_at, props.now)}
      </span>
    </li>
  );
}

export default function TeamQueue({ team, bots, now }: TeamQueueProps): ReactElement {
  const names = new Map(bots.map((b) => [b.id, b.name]));
  return (
    <section className="team-queue" aria-label="Running bots">
      <p className="team-running">
        <span className="team-running-count">{runningText(team)}</span>
        <span className="team-line">
          on this computer · idle bots sleep after {team.idle_sleep_minutes} min
        </span>
      </p>
      {team.queue.length === 0 ? null : (
        <ol className="team-queue-list" aria-label="Waiting for a slot">
          {team.queue.map((wake) => (
            <QueueRow
              key={wake.bot_id}
              wake={wake}
              name={names.get(wake.bot_id) ?? wake.bot_id}
              now={now}
            />
          ))}
        </ol>
      )}
    </section>
  );
}

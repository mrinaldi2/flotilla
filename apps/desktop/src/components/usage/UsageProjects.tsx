// The Usage page's "By project" section (0.24.0, `usage_v1`): each project's
// tokens for the period and, when it has one, its weekly budget as a bar with
// its state in words; then the tokens per day of the period.

import type { ReactElement } from "react";
import type { Project } from "../../protocol/entities";
import type { Usage, UsageRange } from "../../protocol/usage";
import {
  budgetShare,
  budgetState,
  budgetUsed,
  costText,
  dayLabel,
  percentText,
  tokensText,
} from "./usageText";

type ProjectUsage = Usage["projects"][number];
type Budget = NonNullable<ProjectUsage["budget"]>;

function BudgetBar(props: { readonly budget: Budget; readonly now: number }): ReactElement {
  const { budget, now } = props;
  const share = budgetShare(budget);
  const state = budgetState(budget, now);
  return (
    <>
      <div className="usage-budget-line">
        <span>
          {budgetUsed(budget.used, budget.weekly_tokens)} · {percentText(share)}
        </span>
        <span className={`usage-state usage-state-${state.tone}`}>{state.words}</span>
      </div>
      <div className="usage-bar" aria-hidden="true">
        <div
          className={`usage-bar-fill usage-bar-${state.tone}`}
          style={{ width: `${Math.min(100, share * 100)}%` }}
        />
      </div>
      <p className="usage-dim">
        {budget.reached
          ? "Its on-demand bots don't wake until you raise or extend it; the lead keeps going."
          : "At 100% its on-demand bots stop waking; the lead keeps going."}
      </p>
    </>
  );
}

function ProjectLine(props: {
  readonly item: ProjectUsage;
  readonly now: number;
  /** Null when the project isn't one of this computer's: no budget to set here. */
  readonly onOpenBudget: (() => void) | null;
}): ReactElement {
  const { item, now, onOpenBudget } = props;
  return (
    <li className="usage-project">
      <div className="usage-project-head">
        <span className="usage-name">{item.name}</span>
        <span className="usage-dim">
          {tokensText(item.totals.tokens)} · {percentText(item.share)} ·{" "}
          {costText(item.totals.est_cost_usd)}
        </span>
        {onOpenBudget === null ? null : (
          <button
            type="button"
            className="btn btn-small usage-budget-btn"
            aria-label={`${item.budget === undefined ? "Set" : "Change"} ${item.name}'s budget`}
            onClick={onOpenBudget}
          >
            {item.budget === undefined ? "Set budget…" : "Change budget…"}
          </button>
        )}
      </div>
      {item.budget === undefined ? (
        <p className="usage-dim">No weekly budget.</p>
      ) : (
        <BudgetBar budget={item.budget} now={now} />
      )}
    </li>
  );
}

/** Flex bars, one per day, tallest the busiest; their values are its label. */
function DayBars(props: {
  readonly days: Usage["days"];
  readonly range: UsageRange;
  readonly now: number;
}): ReactElement | null {
  const { days, range, now } = props;
  if (days.length < 2) {
    return null;
  }
  const most = Math.max(1, ...days.map((d) => d.totals.tokens));
  const labels = days.map((d) => dayLabel(d.date, now, range));
  const spoken = days.map((d, i) => `${labels[i] ?? d.date} ${tokensText(d.totals.tokens)}`);
  const sparse = days.length > 10;
  return (
    <div className="usage-days">
      <span className="usage-dim">Tokens per day, all projects</span>
      <div
        className="usage-days-bars"
        role="img"
        aria-label={`Tokens per day: ${spoken.join(", ")}`}
      >
        {days.map((d, i) => (
          <div key={d.date} className="usage-day">
            <div
              className="usage-day-bar"
              style={{ height: `${Math.max(2, (d.totals.tokens / most) * 100)}%` }}
            />
            <span className="usage-day-label">
              {sparse && i % 5 !== 0 && i !== days.length - 1 ? "\u00a0" : labels[i]}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}

export default function UsageProjects(props: {
  readonly usage: Usage;
  readonly projects: readonly Project[];
  readonly now: number;
  readonly onOpenBudget: (projectId: string) => void;
}): ReactElement {
  const { usage, now } = props;
  const mine = new Set(props.projects.map((p) => p.id));
  return (
    <section className="usage-card usage-by-project" aria-labelledby="usage-projects-title">
      <h2 className="usage-card-title" id="usage-projects-title">
        By project, against its weekly budget
      </h2>
      {usage.projects.length === 0 ? (
        <p className="usage-dim">No project used any tokens in this period.</p>
      ) : (
        <ul className="usage-projects">
          {usage.projects.map((item) => (
            <ProjectLine
              key={item.project_id}
              item={item}
              now={now}
              onOpenBudget={
                mine.has(item.project_id)
                  ? () => {
                      props.onOpenBudget(item.project_id);
                    }
                  : null
              }
            />
          ))}
        </ul>
      )}
      <DayBars days={usage.days} range={usage.range} now={now} />
    </section>
  );
}

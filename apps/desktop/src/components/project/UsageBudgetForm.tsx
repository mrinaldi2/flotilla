// A project's usage budget (0.24.0, `usage_v1`): the Claude tokens its bots
// may use over the last 7 days. When it runs out, its on-demand bots stop
// waking; the lead and always-on bots keep going. The owner's alone.

import { useEffect, useRef, useState } from "react";
import type { FormEvent, ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { NotifyLevel, Project } from "../../protocol/entities";
import { USAGE_V1 } from "../../protocol/usage";
import type { ProjectBudget } from "../../protocol/usage";
import { errText } from "../../util";
import { budgetShare, budgetTone, budgetUsed, percentText, tokensText } from "../usage/usageText";

const MILLION = 1e6;

interface UsageBudgetFormProps {
  readonly client: DaemonApi;
  readonly project: Project;
  readonly connected: boolean;
  readonly onToast: (level: NotifyLevel, title: string, body: string) => void;
  /** Opened from the Usage page or a budget row: focus this panel. */
  readonly focus?: boolean;
}

/** "40" for 40 M tokens; empty for no budget. */
function draftOf(budget: ProjectBudget | null | undefined): string {
  return budget == null ? "" : String(Math.round((budget.weekly_tokens / MILLION) * 100) / 100);
}

/** Whole tokens for a draft in millions, or undefined when it isn't a budget. */
function tokensOf(draft: string): number | undefined {
  const millions = Number(draft.trim());
  if (draft.trim() === "" || !Number.isFinite(millions) || millions > 1_000_000) {
    return undefined;
  }
  const tokens = Math.round(millions * MILLION);
  return tokens > 0 ? tokens : undefined;
}

/** Focuses the panel's heading when asked to. */
function useFocusOn(focus: boolean) {
  const heading = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (focus) {
      heading.current?.focus();
      heading.current?.scrollIntoView?.({ block: "start" });
    }
  }, [focus]);
  return heading;
}

function UsedBar({ budget }: { readonly budget: ProjectBudget }): ReactElement {
  const share = budgetShare(budget);
  const tone = budgetTone(budget);
  return (
    <div className="usage-budget-used">
      <span className="field-hint">
        {budgetUsed(budget.used, budget.weekly_tokens)} used · {percentText(share)} · last 7 days
        {budget.reached ? " · reached: its on-demand bots are held" : ""}
      </span>
      <div className="usage-bar" aria-hidden="true">
        <div
          className={`usage-bar-fill usage-bar-${tone}`}
          style={{ width: `${Math.min(100, share * 100)}%` }}
        />
      </div>
    </div>
  );
}

function UsageBudgetForm(props: UsageBudgetFormProps): ReactElement {
  const { client, project, connected, onToast } = props;
  const saved = draftOf(project.budget);
  const [draft, setDraft] = useState(saved);
  const [saving, setSaving] = useState(false);
  const heading = useFocusOn(props.focus === true);
  const owner = client.hasGrant("approve");
  const tokens = tokensOf(draft);
  const dirty = draft.trim() !== saved;
  const off = !connected || saving || !owner;

  const send = async (weekly: number | null): Promise<void> => {
    setSaving(true);
    try {
      const reply = await client.request(
        { type: "set_project_budget", project_id: project.id, weekly_tokens: weekly },
        "project",
      );
      setDraft(draftOf(reply.project.budget));
      onToast(
        "info",
        weekly === null ? "No usage budget" : "Usage budget set",
        weekly === null
          ? `${project.name}'s bots may use what they need.`
          : `${project.name}'s bots may use ${tokensText(weekly)} tokens a week.`,
      );
    } catch (error) {
      onToast("error", "Couldn't set the usage budget", errText(error));
    } finally {
      setSaving(false);
    }
  };
  const save = (event: FormEvent): void => {
    event.preventDefault();
    if (tokens !== undefined && dirty && !off) {
      void send(tokens);
    }
  };

  return (
    <form className="panel" aria-labelledby="usage-budget-title" onSubmit={save}>
      <h3 className="panel-title" id="usage-budget-title" ref={heading} tabIndex={-1}>
        Usage budget
      </h3>
      <label className="field">
        <span className="field-label">Weekly budget, in million tokens</span>
        <input
          type="text"
          inputMode="decimal"
          placeholder="No budget"
          value={draft}
          disabled={!owner}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
        />
        <span className="field-hint">
          When it runs out, on-demand bots stop waking; the lead and always-on bots keep going.
          {owner ? "" : " Only the owner sets this."}
        </span>
      </label>
      {project.budget == null ? null : <UsedBar budget={project.budget} />}
      {draft.trim() !== "" && tokens === undefined ? (
        <p className="field-error" role="alert">
          Enter millions of tokens above 0, such as 40 or 2.5.
        </p>
      ) : null}
      {owner ? (
        <div className="panel-actions">
          <button
            type="submit"
            className="btn btn-primary"
            disabled={off || !dirty || tokens === undefined}
          >
            {saving ? "Saving…" : "Save"}
          </button>
          {project.budget == null ? null : (
            <button
              type="button"
              className="btn"
              disabled={off}
              onClick={() => {
                void send(null);
              }}
            >
              No budget
            </button>
          )}
        </div>
      ) : null}
    </form>
  );
}

/** The form on a service with `usage_v1`, fresh whenever the budget changes. */
export default function UsageBudget(props: UsageBudgetFormProps): ReactElement | null {
  const { project } = props;
  return props.client.capabilities.includes(USAGE_V1) ? (
    <UsageBudgetForm key={`${project.id}:${project.budget?.weekly_tokens ?? ""}`} {...props} />
  ) : null;
}

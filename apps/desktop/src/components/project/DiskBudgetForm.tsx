// A project's disk budget (0.23.0, `repo_mirror_v1`): what its bots'
// worktrees, build caches and repository mirror may use on this computer.
// Over it, the hourly disk report cleans merged cards' worktrees first and
// tells the owner about the rest. The owner's alone.

import { useCallback, useState } from "react";
import type { FormEvent, ReactElement } from "react";
import { useLoadOnConnect } from "../../hooks/useLoadOnConnect";
import type { DaemonApi } from "../../protocol/api";
import { sizeText } from "../../protocol/cleanup";
import type { DiskBudget } from "../../protocol/cleanup";
import type { NotifyLevel, Project } from "../../protocol/entities";
import { mirrorOn } from "../../protocol/team";
import { errText } from "../../util";

const GB = 1e9;

interface DiskBudgetFormProps {
  readonly client: DaemonApi;
  readonly project: Project;
  readonly connected: boolean;
  readonly canControl: boolean;
  readonly onToast: (level: NotifyLevel, title: string, body: string) => void;
}

/** "20" for 20 GB; empty for no budget. */
function draftOf(bytes: number | null | undefined): string {
  return bytes == null ? "" : String(Math.round((bytes / GB) * 10) / 10);
}

/** Bytes for a draft in GB, null for none, or undefined when it isn't a size. */
function bytesOf(draft: string): number | null | undefined {
  const trimmed = draft.trim();
  if (trimmed === "") {
    return null;
  }
  const gb = Number(trimmed);
  return Number.isFinite(gb) && gb > 0 && gb <= 100_000 ? Math.round(gb * GB) : undefined;
}

/** This project's line in the last disk report, when it has a budget. */
function useBudgetUse(client: DaemonApi, projectId: string, connected: boolean) {
  const [use, setUse] = useState<DiskBudget | null>(null);
  const load = useCallback(async (): Promise<void> => {
    try {
      const reply = await client.request({ type: "disk_report" }, "disk_report");
      setUse(reply.disk_report.budgets?.find((b) => b.project_id === projectId) ?? null);
    } catch {
      // No report yet: the budget is still set, just not measured.
    }
  }, [client, projectId]);
  useLoadOnConnect(connected, load);
  return use;
}

function UseLine({ use }: { readonly use: DiskBudget }): ReactElement {
  const over = use.used_bytes > use.budget_bytes;
  return (
    <p className={over ? "field-error" : "field-hint"}>
      {over ? "Over budget: " : "Uses "}
      {sizeText(use.used_bytes)} of {sizeText(use.budget_bytes)}
      {use.card_worktree_bytes > 0
        ? `, ${sizeText(use.card_worktree_bytes)} of it in card worktrees`
        : ""}
      , as of the last disk report.
    </p>
  );
}

function DiskBudgetForm(props: DiskBudgetFormProps): ReactElement {
  const { client, project, connected, canControl, onToast } = props;
  const saved = draftOf(project.disk_budget_bytes);
  const [draft, setDraft] = useState(saved);
  const [saving, setSaving] = useState(false);
  const use = useBudgetUse(client, project.id, connected);
  const bytes = bytesOf(draft);
  const dirty = draft.trim() !== saved;

  const save = async (event: FormEvent): Promise<void> => {
    event.preventDefault();
    if (bytes === undefined || !dirty) {
      return;
    }
    setSaving(true);
    try {
      const reply = await client.request(
        { type: "set_project_disk_budget", project_id: project.id, bytes },
        "project",
      );
      setDraft(draftOf(reply.project.disk_budget_bytes));
      onToast(
        "info",
        bytes === null ? "No disk budget" : "Disk budget set",
        bytes === null
          ? `${project.name} may use what it needs.`
          : `${project.name} may use ${sizeText(bytes)} for its worktrees and caches.`,
      );
    } catch (error) {
      onToast("error", "Couldn't set the disk budget", errText(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <form className="panel" aria-labelledby="disk-budget-title" onSubmit={(e) => void save(e)}>
      <h3 className="panel-title" id="disk-budget-title">
        Disk budget
      </h3>
      <label className="field">
        <span className="field-label">Worktrees and build caches, in GB</span>
        <input
          type="text"
          inputMode="decimal"
          placeholder="No budget"
          value={draft}
          disabled={!canControl}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
        />
        <span className="field-hint">
          Over budget, the hourly disk report removes the worktrees of merged cards first, then
          tells you. Leave empty for no budget. Only you set this, from this computer.
        </span>
      </label>
      {use === null ? null : <UseLine use={use} />}
      {bytes === undefined ? (
        <p className="field-error" role="alert">
          Enter a size in GB, such as 20, or leave it empty.
        </p>
      ) : null}
      {canControl ? (
        <div className="panel-actions">
          <button
            type="submit"
            className="btn btn-primary"
            disabled={!connected || saving || !dirty || bytes === undefined}
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      ) : null}
    </form>
  );
}

/** The form on a service with `repo_mirror_v1`, fresh whenever the budget changes. */
export default function DiskBudget(props: DiskBudgetFormProps): ReactElement | null {
  const { project } = props;
  return mirrorOn(props.client) ? (
    <DiskBudgetForm key={`${project.id}:${project.disk_budget_bytes ?? ""}`} {...props} />
  ) : null;
}

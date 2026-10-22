// Needs-you rows of 0.24.0: a column assigns a role nobody holds
// (`board_flow_v1`), offering Create a bot with it; and a project near or
// past its weekly token budget (`usage_v1`), offering Raise budget and, once
// it's used up, Extend today.

import type { AttentionRowJson } from "../../protocol/dashboard";
import type { Role } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { roleLabel } from "../bot/roleLabels";
import { roleOfToken } from "../team/createRoleBot";
import type { RowViewProps } from "./DashRow";

export interface FlowRowActions {
  /** Creates a bot with the role; absent on a dashboard that can't. */
  readonly onCreateRoleBot?: (role: Role) => void;
  /** The role whose bot is being created. */
  readonly creatingRole?: Role | null;
  /** Opens the project's Settings on its budget. */
  readonly onRaiseBudget?: (projectId: string) => void;
  /** Extends a used-up budget until local midnight (`budget_extend`). */
  readonly onExtendBudget?: (projectId: string) => void;
  /** Roles and budgets are the owner's: the approve grant, while connected. */
  readonly canApprove?: boolean;
  /** The project an extension is on its way for. */
  readonly extending?: string | null;
}

const OWNER_ONLY = "Only the owner can, from a device with approve access";

/** "Nobody holds Architect: CASS-7 waits in Research": Create a bot. */
function roleUnheld(r: AttentionRowJson, a: FlowRowActions): RowViewProps {
  const role = roleOfToken(r.role);
  const create = a.onCreateRoleBot;
  const label = role === undefined ? "the role" : roleLabel(role);
  const creating = (a.creatingRole ?? null) !== null;
  const owner = a.canApprove === true;
  return {
    glyph: "▲",
    tone: "you",
    title: r.title,
    meta: creating ? "Creating the bot…" : "Its cards stay with whoever moved them",
    action: role === undefined || create === undefined ? undefined : "Create a bot",
    label: `Create a bot with ${label}`,
    onAction: () => {
      if (role !== undefined) {
        create?.(role);
      }
    },
    disabled: !owner || creating,
    why: owner ? undefined : OWNER_ONLY,
    verb: "Create a bot",
  };
}

/** Raise budget…, when the dashboard can open the project's Settings. */
function raise(r: AttentionRowJson, a: FlowRowActions) {
  const project = r.project_id;
  const open = a.onRaiseBudget;
  if (project === undefined || project.length === 0 || open === undefined) {
    return undefined;
  }
  return {
    action: "Raise budget…",
    label: `Raise budget: ${r.title}`,
    onAction: () => open(project),
  };
}

/** "PhD used 80% of its weekly budget": Raise budget…. */
function budgetWarning(r: AttentionRowJson, a: FlowRowActions): RowViewProps {
  const up = raise(r, a);
  return {
    glyph: "▲",
    tone: "you",
    title: r.title,
    meta: "At 100% its on-demand bots stop waking",
    action: up?.action,
    label: up?.label,
    onAction: up?.onAction,
    verb: "Raise budget",
  };
}

/** "PhD used its weekly budget": Extend today, or Raise budget…. */
function budgetReached(r: AttentionRowJson, a: FlowRowActions): RowViewProps {
  const project = r.project_id ?? "";
  const extend = a.onExtendBudget;
  const busy = a.extending === project;
  const owner = a.canApprove === true;
  return {
    glyph: "✗",
    tone: "bad",
    title: r.title,
    meta: busy
      ? "Extending it for today…"
      : "Its on-demand bots wait as Budget reached; the lead keeps going",
    action: project.length === 0 || extend === undefined ? undefined : "Extend today",
    label: `Extend today: ${r.title}`,
    onAction: () => extend?.(project),
    disabled: !owner || busy,
    why: owner ? undefined : OWNER_ONLY,
    verb: "Extend",
    secondary: raise(r, a),
  };
}

/** The row for a 0.24 kind; undefined for any other. */
export function flowRow(r: AttentionRowJson, a: FlowRowActions): RowViewProps | undefined {
  switch (r.kind) {
    case "role_unheld":
      return roleUnheld(r, a);
    case "budget_warning":
      return budgetWarning(r, a);
    case "budget_reached":
      return budgetReached(r, a);
    default:
      return undefined;
  }
}

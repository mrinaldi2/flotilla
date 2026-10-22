// Needs-you rows of 0.24.0, as a daemon raises them: a column's role nobody
// holds (`board_flow_v1`), and a project near and past its weekly budget
// (`usage_v1`).

import type { AttentionRowJson } from "../protocol/dashboard";

export const ROLE_UNHELD: AttentionRowJson = {
  kind: "role_unheld",
  id: "role_unheld:d-mac:p1/main:architect",
  title: "Nobody holds Architect: CASS-7 waits in Research",
  role: "architect",
};

export const BUDGET_WARNING: AttentionRowJson = {
  kind: "budget_warning",
  id: "budget_warning:d-mac:p1",
  title: "PhD used 96% of its weekly budget",
  project_id: "p1",
};

export const BUDGET_REACHED: AttentionRowJson = {
  kind: "budget_reached",
  id: "budget_reached:d-mac:p2",
  title: "Books used its weekly budget: its on-demand bots are held",
  project_id: "p2",
};

export const FLOW_ROWS: readonly AttentionRowJson[] = [ROLE_UNHELD, BUDGET_WARNING, BUDGET_REACHED];

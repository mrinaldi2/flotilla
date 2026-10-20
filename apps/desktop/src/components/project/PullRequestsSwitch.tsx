// Whether the project's code goes through pull requests (0.23.1): bots push
// their branch and open its PR with `pr_open`, and the change is reviewed and
// merged from the PR. The bots' instructions change from their next start.
// The owner's alone.

import { useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { NotifyLevel, Project } from "../../protocol/entities";
import { errText } from "../../util";

interface PullRequestsSwitchProps {
  readonly client: DaemonApi;
  readonly project: Project;
  readonly connected: boolean;
  readonly canControl: boolean;
  readonly onToast: (level: NotifyLevel, title: string, body: string) => void;
}

export default function PullRequestsSwitch(props: PullRequestsSwitchProps): ReactElement | null {
  const { client, project, connected, canControl, onToast } = props;
  const [on, setOn] = useState(project.pr_flow === true);
  const [saving, setSaving] = useState(false);
  if (project.pr_flow === undefined) {
    return null;
  }
  const flip = async (enabled: boolean): Promise<void> => {
    setSaving(true);
    try {
      const reply = await client.request(
        { type: "set_project_pr_flow", project_id: project.id, enabled },
        "project",
      );
      setOn(reply.project.pr_flow === true);
      onToast(
        "info",
        enabled ? "Pull requests on" : "Pull requests off",
        enabled
          ? `${project.name}'s bots open a pull request for their code, from their next start.`
          : `${project.name}'s bots no longer open pull requests, from their next start.`,
      );
    } catch (error) {
      onToast("error", "Couldn't change pull requests", errText(error));
    } finally {
      setSaving(false);
    }
  };
  return (
    <section className="panel" aria-labelledby="pr-flow-title">
      <h3 className="panel-title" id="pr-flow-title">
        Pull requests
      </h3>
      <label className="checkbox-row">
        <input
          type="checkbox"
          checked={on}
          disabled={!canControl || !connected || saving}
          onChange={(event) => void flip(event.target.checked)}
        />
        Code goes through pull requests
      </label>
      <p className="field-hint">
        Bots push their branch and open its pull request on the card, and the change is reviewed and
        merged from it. Off, a code card still needs its pushed branch linked to reach Review. Only
        you set this, from this computer.
      </p>
    </section>
  );
}

import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { Project } from "../../protocol/entities";
import * as fx from "../../test/fixtures";
import { toastSpy } from "../../test/spies";
import { teamDaemon } from "../../test/teamFixtures";
import PullRequestsSwitch from "./PullRequestsSwitch";

function renderSwitch(project: Project) {
  const sent: boolean[] = [];
  const client = teamDaemon().onRequest("set_project_pr_flow", (body) => {
    const enabled = body.type === "set_project_pr_flow" && body.enabled;
    sent.push(enabled);
    return { type: "project", req_id: "1", project: { ...project, pr_flow: enabled } };
  });
  const onToast = toastSpy();
  render(
    <PullRequestsSwitch client={client} project={project} connected canControl onToast={onToast} />,
  );
  return { sent, onToast };
}

describe("a project's pull requests switch (0.23.1)", () => {
  it("turns pull requests on for the project's bots", async () => {
    const user = userEvent.setup();
    const { sent, onToast } = renderSwitch(fx.project({ pr_flow: false }));
    const box = screen.getByRole("checkbox", { name: "Code goes through pull requests" });
    expect(box).not.toBeChecked();
    await user.click(box);
    expect(sent).toEqual([true]);
    expect(box).toBeChecked();
    expect(onToast).toHaveBeenCalledWith("info", "Pull requests on", expect.any(String));
  });

  it("is hidden on a service from before 0.23.1", () => {
    renderSwitch(fx.project({}));
    expect(screen.queryByRole("checkbox")).toBeNull();
  });
});

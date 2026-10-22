import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { OwnerAction } from "../../protocol/ownerActions";
import { FakeDaemon } from "../../test/fakeDaemon";
import { ownerAction, PUSH_NOW, pushAction } from "../../test/ownerActionFixtures";
import { actionToastSpy } from "../../test/spies";
import OwnerActionList from "./OwnerActionList";
import { expiresIn, pushResultLine } from "./ownerActionText";

function daemon(actions: readonly OwnerAction[], owner = true): FakeDaemon {
  const first = actions[0] ?? pushAction();
  const client = new FakeDaemon()
    .onRequest("owner_action_list", () => ({ type: "owner_actions", req_id: "1", actions }))
    .onRequest("owner_action_run", () => ({
      type: "owner_action",
      req_id: "1",
      action: { ...first, state: "running" },
    }))
    .onRequest("owner_action_reject", () => ({
      type: "owner_action",
      req_id: "1",
      action: { ...first, state: "rejected", reject_reason: "not yet" },
    }));
  client.grants = owner ? ["read", "control", "approve"] : ["read", "control"];
  return client;
}

function renderList(client: FakeDaemon, title?: string) {
  return render(
    <OwnerActionList
      client={client}
      connected
      scope={{ projectId: "p1", waitingOnly: true }}
      addToast={actionToastSpy()}
      botName={(id) => (id === "tester" ? "Automation Tester" : "a bot")}
      title={title}
      pushTitle={title === undefined ? undefined : "Commands and pushes for you"}
      now={PUSH_NOW}
    />,
  );
}

async function card(): Promise<HTMLElement> {
  return screen.findByRole("article", {
    name: "Automation Tester asks you to push: automation-tester/tour-lock-stack",
  });
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("a push request's card", () => {
  it("shows the exact command, what it pushes and why", async () => {
    renderList(daemon([pushAction()]));
    const push = await card();
    expect(push).toHaveTextContent("expires in 23 h");
    expect(within(push).getByText(pushAction().content)).toBeInTheDocument();
    const shown = within(push);
    expect(shown.getByText("automation-tester/tour-lock-stack")).toBeInTheDocument();
    expect(shown.getByText("a334dd34 · 2 files, +208 −3")).toBeInTheDocument();
    expect(shown.getByText("acme/cass")).toBeInTheDocument();
    expect(shown.getByText("Card")).toBeInTheDocument();
    expect(shown.getByText("CASS-053")).toBeInTheDocument();
    expect(within(push).getByText("“Ready for review; self-test 20/20.”")).toBeInTheDocument();
    expect(push).toHaveTextContent("Pushes to main are never offered here.");
  });

  it("names the target when it isn't the branch, and one file as one", async () => {
    const base = pushAction();
    const push = base.push;
    if (push === undefined) {
      throw new Error("no push");
    }
    renderList(daemon([pushAction({ push: { ...push, target: "review/tour-lock", files: 1 } })]));
    const shown = await card();
    expect(shown).toHaveTextContent("automation-tester/tour-lock-stack → review/tour-lock");
    expect(shown).toHaveTextContent("1 file, +208 −3");
  });

  it("pushes in one click, sending the hash it showed", async () => {
    const user = userEvent.setup();
    const client = daemon([pushAction()]);
    renderList(client);
    await user.click(within(await card()).getByRole("button", { name: "Push" }));
    await waitFor(() => {
      expect(client.requests.at(-1)?.body).toEqual({
        type: "owner_action_run",
        id: "oa-push",
        sha256: pushAction().sha256,
      });
    });
    expect(await screen.findByText("◑ Pushing…")).toBeInTheDocument();
  });

  it("rejects with a reason", async () => {
    const user = userEvent.setup();
    const client = daemon([pushAction()]);
    renderList(client);
    await user.click(within(await card()).getByRole("button", { name: "Reject…" }));
    await user.type(screen.getByRole("textbox", { name: "Why not (optional)" }), "not yet");
    await user.click(screen.getByRole("button", { name: "Reject" }));
    expect(client.requests.at(-1)?.body).toEqual({
      type: "owner_action_reject",
      id: "oa-push",
      reason: "not yet",
    });
  });

  it("opens the diff on GitHub, only for an owner/name repository", async () => {
    const user = userEvent.setup();
    const open = vi.spyOn(window, "open").mockReturnValue(null);
    renderList(daemon([pushAction()]));
    await user.click(within(await card()).getByRole("button", { name: "See the diff" }));
    expect(open).toHaveBeenCalledWith(
      "https://github.com/acme/cass/compare/automation-tester/tour-lock-stack...a334dd34c0ffee5e1d2b3a4c5d6e7f8091a2b3c4",
      "_blank",
      "noopener,noreferrer",
    );
  });

  it("hides the diff for a repository that isn't on GitHub", async () => {
    const push = pushAction().push;
    if (push === undefined) {
      throw new Error("no push");
    }
    renderList(daemon([pushAction({ push: { ...push, repo: "ssh://git.example.com/cass.git" } })]));
    expect(within(await card()).queryByRole("button", { name: "See the diff" })).toBeNull();
  });

  it("says where to push it without the approve grant", async () => {
    renderList(daemon([pushAction()], false));
    const push = await card();
    expect(within(push).queryByRole("button", { name: "Push" })).toBeNull();
    expect(push).toHaveTextContent("You can push this from a device with approve access.");
  });

  it("titles the dashboard's list for pushes once one is listed", async () => {
    const { unmount } = renderList(
      daemon([pushAction(), ownerAction()]),
      "Commands for you to run",
    );
    expect(
      await screen.findByRole("heading", { name: "Commands and pushes for you" }),
    ).toBeVisible();
    unmount();
    renderList(daemon([ownerAction({ item_id: null })]), "Commands for you to run");
    expect(await screen.findByRole("heading", { name: "Commands for you to run" })).toBeVisible();
  });
});

describe("a push request's words", () => {
  const at = Date.parse("2025-03-05T14:02:00Z");

  it("counts down hours, then minutes, then nothing", () => {
    const push = pushAction();
    expect(expiresIn(push, at - 23 * 3_600_000 - 59 * 60_000)).toBe("expires in 23 h");
    expect(expiresIn(push, at - 40 * 60_000)).toBe("expires in 40 min");
    expect(expiresIn(push, at - 10_000)).toBe("expires in 1 min");
    expect(expiresIn(push, at)).toBeNull();
    expect(expiresIn(pushAction({ expires_at: "soon" }), at)).toBeNull();
  });

  it("says how it went in push words", () => {
    const who = "Automation Tester";
    expect(pushResultLine(pushAction({ state: "succeeded" }), who)).toBe(
      "✓ Pushed. Automation Tester goes on from here.",
    );
    expect(pushResultLine(pushAction({ state: "failed", exit_code: 1 }), who)).toBe(
      "✗ The push failed · exit 1. Automation Tester has been told.",
    );
    expect(pushResultLine(pushAction({ state: "failed" }), who)).toBe(
      "✗ The push failed. Automation Tester has been told.",
    );
    expect(pushResultLine(pushAction({ state: "rejected", reject_reason: "no" }), who)).toBe(
      "⊘ You rejected it: no",
    );
    expect(pushResultLine(pushAction(), who)).toBeNull();
  });
});

import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ProjectTab } from "../../app/selection";
import type { AddToast } from "../../app/useToasts";
import { DASH_BOTS, dashboard } from "../../test/dashboardFixtures";
import type { FakeDaemon } from "../../test/fakeDaemon";
import { project } from "../../test/fixtures";
import {
  calendarQuestion,
  QUESTION_ROWS,
  questionsClient,
  saintsQuestion,
} from "../../test/questionFixtures";
import DashboardView from "./DashboardView";

function setup(fake: FakeDaemon = questionsClient([saintsQuestion(), calendarQuestion()])) {
  const data = dashboard();
  let reads = 0;
  fake.onRequest("dashboard_get", () => {
    reads += 1;
    return {
      type: "dashboard",
      req_id: "1",
      dashboard: { ...data, needs_you: [...QUESTION_ROWS, ...data.needs_you] },
    };
  });
  render(
    <DashboardView
      client={fake}
      project={project({ id: "p1", name: "Saints" })}
      bots={DASH_BOTS}
      connected
      canControl
      addToast={vi.fn<AddToast>()}
      onOpenTab={vi.fn<(tab: ProjectTab) => void>()}
      onOpenBot={vi.fn<(botId: string) => void>()}
      onOpenDecision={vi.fn<(decisionId: string) => void>()}
    />,
  );
  return { fake, reads: () => reads };
}

describe("questions in Needs you", () => {
  it("lists a blocking question first, with who asked and on which card", async () => {
    setup();
    const needs = await screen.findByRole("region", { name: /Needs you/ });
    const rows = within(needs).getAllByRole("listitem");
    const blocking = rows.findIndex((r) => r.textContent?.includes("cover only the"));
    const other = rows.findIndex((r) => r.textContent?.includes("national calendar"));
    expect(blocking).toBeGreaterThanOrEqual(0);
    expect(blocking).toBeLessThan(other);
    expect(rows[blocking]).toHaveTextContent("Holds the gate · Lead asks you on ASL-4");
    expect(
      within(needs)
        .getByText(/national calendar/)
        .closest("li"),
    ).toHaveTextContent("Lead asks you on ASL-4");
  });

  it("answers a question under its row, then reads Needs you again", async () => {
    const user = userEvent.setup();
    const { fake, reads } = setup();
    const row = (await screen.findByText(/cover only the/)).closest("li");
    if (row === null) {
      throw new Error("no row");
    }
    await user.click(within(row).getByRole("button", { name: /^Answer: Should v1/ }));
    const reply = await within(row).findByRole("textbox", { name: /Your answer/ });
    expect(reply).toHaveFocus();
    expect(fake.boardCalls).toContainEqual({
      case: "questionList",
      value: {
        projectId: "p1",
        itemId: "ASL-4",
        planNodeId: undefined,
        state: 1,
        before: undefined,
        limit: 200,
      },
    });
    const before = reads();
    await user.click(within(row).getByRole("button", { name: /The ~250 celebrated saints/ }));
    await user.click(within(row).getByRole("button", { name: "Answer" }));
    await waitFor(() => expect(reads()).toBeGreaterThan(before));
    await user.click(within(row).getByRole("button", { name: "Close the question" }));
    expect(within(row).queryByRole("textbox")).toBeNull();
  });

  it("says when the question was settled meanwhile", async () => {
    const user = userEvent.setup();
    setup(questionsClient([calendarQuestion()]));
    const row = (await screen.findByText(/cover only the/)).closest("li");
    if (row === null) {
      throw new Error("no row");
    }
    await user.click(within(row).getByRole("button", { name: /^Answer:/ }));
    expect(await within(row).findByRole("status")).toHaveTextContent(
      "It was answered or dismissed meanwhile.",
    );
  });

  it("offers no answer without questions_v1", async () => {
    const fake = questionsClient();
    fake.capabilities = [];
    setup(fake);
    const row = (await screen.findByText(/cover only the/)).closest("li");
    expect(row).not.toBeNull();
    expect(within(row ?? document.body).queryByRole("button")).toBeNull();
  });
});

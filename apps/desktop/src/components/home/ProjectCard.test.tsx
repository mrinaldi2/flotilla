import { create } from "@bufbuild/protobuf";
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  ProjectRowSchema,
  ProjectUsageSchema,
  type ProjectUsage,
} from "../../protocol/gen/flotilla/home/v1/home_pb";
import { HOME_NOW } from "../../test/homeFixtures";
import ProjectCard from "./ProjectCard";

function renderCard(usage: ProjectUsage | undefined): void {
  render(
    <ProjectCard
      row={create(ProjectRowSchema, { projectId: "p1", name: "PhD", bots: 3, usage })}
      sources={[]}
      place={undefined}
      top={false}
      leadName="Lead"
      now={HOME_NOW}
      canPin={false}
      onOpen={vi.fn<(projectId: string) => void>()}
      onPin={vi.fn<(projectId: string, pinned: boolean) => void>()}
    />,
  );
}

describe("a project card's usage line", () => {
  it("says today's tokens and estimate, and the week against its budget", () => {
    renderCard(
      create(ProjectUsageSchema, {
        tokensToday: 1_200_000n,
        tokensWeek: 29_800_000n,
        budgetWeek: 40_000_000n,
        estCostTodayUsd: 3.4,
      }),
    );
    expect(
      screen.getByText("Today 1.2 M tokens · ≈ $3.40 · week 29.8 M of 40 M"),
    ).toBeInTheDocument();
    expect(screen.queryByText("Budget reached")).not.toBeInTheDocument();
  });

  it("says the week alone without a budget, and when the budget is reached", () => {
    renderCard(
      create(ProjectUsageSchema, {
        tokensToday: 940_000n,
        tokensWeek: 10_400_000n,
        budgetReached: true,
      }),
    );
    expect(screen.getByText(/^Today 940 k tokens · ≈ \$0.00 · week 10.4 M/)).toBeInTheDocument();
    expect(screen.getByText("Budget reached")).toHaveClass("home-usage-reached");
  });

  it("has no usage line from a service that doesn't count tokens", () => {
    renderCard(undefined);
    expect(screen.queryByText(/^Today /)).not.toBeInTheDocument();
  });
});

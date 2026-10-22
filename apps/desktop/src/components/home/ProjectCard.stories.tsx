import { create } from "@bufbuild/protobuf";
import type { Story } from "@ladle/react";
import { ProjectRowSchema, ProjectUsageSchema } from "../../protocol/gen/flotilla/home/v1/home_pb";
import { USAGE_NOW } from "../../test/usageFixtures";
import ProjectCard from "./ProjectCard";

const noop = (): void => {};

const ROWS = [
  create(ProjectRowSchema, {
    projectId: "p1",
    name: "PhD",
    bots: 4,
    botsWorking: 2,
    openTasks: 3,
    usage: create(ProjectUsageSchema, {
      tokensToday: 1_200_000n,
      tokensWeek: 29_800_000n,
      budgetWeek: 40_000_000n,
      estCostTodayUsd: 3.4,
    }),
  }),
  create(ProjectRowSchema, {
    projectId: "p2",
    name: "Books",
    bots: 3,
    openTasks: 1,
    usage: create(ProjectUsageSchema, {
      tokensToday: 940_000n,
      tokensWeek: 10_200_000n,
      budgetWeek: 10_000_000n,
      estCostTodayUsd: 1.42,
      budgetReached: true,
    }),
  }),
];

/** Each card's usage today; Books used its weekly budget. */
export const WithUsage: Story = () => (
  <div className="home-scroll" style={{ width: 960 }}>
    <div className="home-grid">
      {ROWS.map((row) => (
        <ProjectCard
          key={row.projectId}
          row={row}
          sources={[]}
          place={undefined}
          top={false}
          leadName="Lead"
          now={USAGE_NOW}
          canPin
          onOpen={noop}
          onPin={noop}
        />
      ))}
    </div>
  </div>
);

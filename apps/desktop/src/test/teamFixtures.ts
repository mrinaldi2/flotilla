// The team on demand (0.23.0) for tests and stories: a daemon that serves
// it, and a running count with a queue at fixed times.

import type { TeamStatus } from "../protocol/entities";
import { REPO_MIRROR_V1, TEAM_ON_DEMAND_V1 } from "../protocol/team";
import { FakeDaemon } from "./fakeDaemon";

/** "Now" for the queue's waits: 12:00 on the fixtures' day. */
export const TEAM_NOW = Date.parse("2024-05-01T12:00:00.000Z");

/** Four of six running, two waiting: an urgent bug ahead of a card. */
export const TEAM_STATUS: TeamStatus = {
  running: 4,
  max_running: 6,
  idle_sleep_minutes: 10,
  queue: [
    {
      bot_id: "b5",
      project_id: "p1",
      reason: "card",
      item_id: "T-31",
      priority: "P0",
      urgent: true,
      queued_at: "2024-05-01T11:58:00.000Z",
    },
    {
      bot_id: "b6",
      project_id: "p1",
      reason: "card",
      item_id: "T-12",
      priority: "P1",
      urgent: false,
      queued_at: "2024-05-01T10:47:00.000Z",
    },
  ],
};

/** A daemon serving the team on demand and the lighter checkouts. */
export function teamDaemon(): FakeDaemon {
  const fake = new FakeDaemon()
    .onRequest("team_status", () => ({ type: "team", req_id: "1", team: TEAM_STATUS }))
    .onRequest("wake_bot", () => ({ type: "ok", req_id: "1" }))
    .onRequest("sleep_bot", () => ({ type: "ok", req_id: "1" }));
  fake.capabilities = [...fake.capabilities, TEAM_ON_DEMAND_V1, REPO_MIRROR_V1];
  return fake;
}

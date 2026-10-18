import { create } from "@bufbuild/protobuf";
import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DaemonError } from "../../protocol/connection";
import { PlanChangeSchema, PlanNodeSchema } from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { FakeDaemon } from "../../test/fakeDaemon";
import { planClient, planTree } from "../../test/planFixtures";
import { nodesWhenOn, planOn, usePlan, usePlanWatch, withNode } from "./usePlan";

describe("a project's plan", () => {
  it("replaces a node by a newer version, keeps a newer one, and adds a new one", () => {
    const tree = planTree();
    const goal = tree.nodes[0];
    if (goal === undefined) {
      throw new Error("the fixture has a goal");
    }
    const newer = create(PlanNodeSchema, { ...goal, title: "Met", version: goal.version + 1n });
    expect(withNode(tree, newer).nodes[0]?.title).toBe("Met");
    const older = create(PlanNodeSchema, { ...goal, title: "Old", version: 0n });
    expect(withNode(tree, older)).toBe(tree);
    const added = create(PlanNodeSchema, { id: "PL-77", title: "New" });
    expect(withNode(tree, added).nodes.at(-1)?.title).toBe("New");
  });

  it("asks nothing of a service without the plan", () => {
    const fake = new FakeDaemon();
    const { result } = renderHook(() => usePlan(fake, "p1", true));
    expect(result.current.status).toEqual({ kind: "unsupported" });
    expect(planOn(result.current.status)).toBe(false);
    expect(fake.boardCalls).toEqual([]);
  });

  it("reads the tree, then follows the project's plan_change pushes", async () => {
    const fake = planClient();
    const { result } = renderHook(() => usePlan(fake, "p1", true));
    await waitFor(() => expect(result.current.status.kind).toBe("ready"));
    expect(planOn(result.current.status)).toBe(true);
    const goal = planTree().nodes[0];
    act(() => {
      fake.emitPlanChange(
        create(PlanChangeSchema, {
          projectId: "p1",
          node: { ...goal, title: "Improve combat feel", version: 9n },
        }),
      );
      fake.emitPlanChange(create(PlanChangeSchema, { projectId: "p2", node: { id: "PL-1" } }));
    });
    expect(nodesWhenOn(result.current.status)?.[0]?.title).toBe("Improve combat feel");
    act(() => result.current.apply(create(PlanNodeSchema, { id: "PL-50", title: "Applied" })));
    expect(nodesWhenOn(result.current.status)?.at(-1)?.title).toBe("Applied");
  });

  it("says why the tree didn't load, and hides the nodes while planning is off", async () => {
    const failing = planClient().onBoard("planGet", () => {
      throw new DaemonError("forbidden", "not yours");
    });
    const failed = renderHook(() => usePlan(failing, "p1", true));
    await waitFor(() =>
      expect(failed.result.current.status).toEqual({ kind: "failed", message: "not yours" }),
    );
    const off = renderHook(() => usePlan(planClient(planTree({ enabled: false })), "p1", true));
    await waitFor(() => expect(off.result.current.status.kind).toBe("ready"));
    expect(nodesWhenOn(off.result.current.status)).toBeUndefined();
  });

  it("watches the project while the tab shows, and stops after", async () => {
    const fake = planClient();
    const { unmount } = renderHook(() => usePlanWatch(fake, "p1", true));
    unmount();
    await waitFor(() =>
      expect(fake.boardCalls.map((c) => c.case)).toEqual(["boardWatch", "boardUnwatch"]),
    );
  });
});

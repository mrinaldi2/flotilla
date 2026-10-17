import { create } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  PlanDocSchema,
  PlanGateSchema,
  PlanKind,
  PlanStage,
} from "../../protocol/gen/flotilla/board/v1/plan_pb";
import { hudEpic, planClient, planNodes } from "../../test/planFixtures";
import PlanGatePanel, { gateWords, roundWords } from "./PlanGatePanel";
import { docState } from "./PlanNodeDetail";
import { stepsOf } from "./PlanStepper";

const gate = (stage: PlanStage) => create(PlanGateSchema, { stage });
const doc = (over: MessageInitShape<typeof PlanDocSchema>) => create(PlanDocSchema, over);

describe("a plan gate", () => {
  it("asks what each kind of gate needs", () => {
    expect(gateWords(gate(PlanStage.PROPOSED))).toMatchObject({
      approve: "Accept",
      sendBack: "Decline",
    });
    expect(gateWords(gate(PlanStage.RESEARCH)).question).toBe("Skip research?");
    expect(gateWords(gate(PlanStage.PRD)).children?.legend).toBe("Epics it creates");
    expect(gateWords(gate(PlanStage.LLD)).children?.legend).toBe("Tasks it creates");
    expect(gateWords(gate(PlanStage.DESIGN))).toEqual({
      question: "Approve the Design?",
      approve: "Approve",
      sendBack: "Send back",
    });
  });

  it("counts the rounds after the first", () => {
    expect(roundWords(1)).toBe("");
    expect(roundWords(2)).toBe("second submission");
    expect(roundWords(4)).toBe("submission 4");
  });

  it("waits for the owner on a connection that can't rule", () => {
    const node = hudEpic();
    render(
      <PlanGatePanel
        api={planClient()}
        node={node}
        gate={node.gate ?? gate(PlanStage.HLD)}
        canRule={false}
        who={() => "Game Lead"}
        onRuled={() => {}}
      />,
    );
    expect(screen.getByText("Waiting for the owner to decide.")).toBeVisible();
    expect(screen.queryByRole("button", { name: /Approve/ })).not.toBeInTheDocument();
    expect(screen.getByText(/Approval is pinned to 4e1c9a2/)).toBeVisible();
  });
});

describe("a node's stages and documents", () => {
  it("marks the stages passed, current and to come", () => {
    const steps = stepsOf(hudEpic());
    expect(steps.map((s) => [s.state, s.sub])).toEqual([
      ["past", "Closed · 2 spikes"],
      ["past", ""],
      ["now", "Waiting for you"],
      ["next", ""],
      ["next", ""],
    ]);
    const lld = planNodes().find((n) => n.kind === PlanKind.FEATURE && n.stage === PlanStage.LLD);
    expect(lld && stepsOf(lld)[0]?.sub).toBe("Being written");
  });

  it("says where each document stands", () => {
    expect(docState(doc({ changedSinceApproval: true, approvedSha: "77ad310" }))).toBe(
      "Changed since approval",
    );
    expect(docState(doc({ approvedSha: "9b2f0c1" }))).toBe("Approved @ 9b2f0c1");
    expect(docState(doc({}))).toBe("Not approved yet");
  });
});

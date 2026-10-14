import { create } from "@bufbuild/protobuf";
import { act, render, renderHook, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { StructureResultSchema } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { boards, boardsSnapshot, proposals, recipeColumns } from "../../../test/boardsFixtures";
import { decision } from "../../../test/decisionFixtures";
import { FakeDaemon } from "../../../test/fakeDaemon";
import ProposalReview from "./ProposalReview";
import { useDecisionProposal } from "./useDecisionProposal";

const open = proposals()[0];
if (open === undefined) {
  throw new Error("the fixture has an open proposal");
}

/** A decision as the Hermes service raises one for a lead's proposal. */
function proposed(over: Parameters<typeof decision>[0]) {
  return decision({
    kind: "question",
    title: "Board change proposed for Recipes: add column “QA”",
    body: "Team Lead proposes to add column “QA” on board Recipes.\n\nApprove or decline it in Needs you; nothing changes until you do. Proposal bp-2.",
    options: [],
    ...over,
  });
}

describe("a lead's proposal in Decisions", () => {
  it("finds the proposal its decision raised, and rules on it", async () => {
    const fake = new FakeDaemon()
      .onBoard("boardGet", () => ({ case: "board", value: boardsSnapshot() }))
      .onBoard("boardProposalRule", () => ({
        case: "structure",
        value: create(StructureResultSchema, { boardId: open.boardId }),
      }));
    const asked = proposed({ id: "dec-2", project_id: "p1", state: "open" });
    const { result } = renderHook(() => useDecisionProposal(fake, true, asked));
    await vi.waitFor(() => expect(result.current.found?.proposal.id).toBe("bp-2"));
    expect(result.current.found?.board?.name).toBe("Recipes");
    expect(result.current.found?.columns.map((c) => c.key)[0]).toBe("rb-todo");
    await act(() => result.current.rule(false, "Not now"));
    const call = fake.boardCalls.at(-1);
    expect(call?.case === "boardProposalRule" && call.value).toMatchObject({
      proposalId: "bp-2",
      approve: false,
      note: "Not now",
    });
    expect(result.current.found).toBeUndefined();
  });

  it("shows no proposal for another decision, or a settled one", async () => {
    const fake = new FakeDaemon().onBoard("boardGet", () => ({
      case: "board",
      value: boardsSnapshot(),
    }));
    const other = renderHook(() =>
      useDecisionProposal(fake, true, proposed({ id: "dec-x", state: "open" })),
    );
    const settled = renderHook(() =>
      useDecisionProposal(fake, true, proposed({ id: "dec-2", state: "settled" })),
    );
    await vi.waitFor(() => expect(fake.boardCalls.length).toBeGreaterThan(0));
    expect(other.result.current.found).toBeUndefined();
    expect(settled.result.current.found).toBeUndefined();
  });

  it("doesn't read a board for another kind of decision, since reading one may start it", async () => {
    const fake = new FakeDaemon().onBoard("boardGet", () => ({
      case: "board",
      value: boardsSnapshot(),
    }));
    const { result } = renderHook(() =>
      useDecisionProposal(fake, true, decision({ id: "dec-2", state: "open" })),
    );
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20));
    });
    expect(fake.boardCalls).toEqual([]);
    expect(result.current.found).toBeUndefined();
  });

  it("approves with the owner's note, and can't without the approve grant", async () => {
    const user = userEvent.setup();
    const onRule = vi.fn<(approve: boolean, note?: string) => void>();
    const found = { proposal: open, board: boards()[1], columns: recipeColumns() };
    const { rerender } = render(
      <ProposalReview
        found={found}
        lead="Team Lead"
        canApprove
        busy={false}
        error={null}
        onRule={onRule}
      />,
    );
    expect(screen.getByText("When PR merged → move to Done")).toBeInTheDocument();
    await user.type(
      screen.getByRole("textbox", { name: "Note for the lead (optional)" }),
      "Good call",
    );
    await user.click(screen.getByRole("button", { name: "Approve" }));
    expect(onRule).toHaveBeenCalledWith(true, "Good call");
    rerender(
      <ProposalReview
        found={found}
        lead="Team Lead"
        canApprove={false}
        busy={false}
        error="nope"
        onRule={onRule}
      />,
    );
    expect(screen.getByRole("button", { name: "Approve" })).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("nope");
  });
});

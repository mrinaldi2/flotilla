import type { Story } from "@ladle/react";
import { create } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ReactElement } from "react";
import { ColumnKind } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { BoardProposalSchema } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import type { BoardProposal } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { boards, RECIPES_ID, recipeColumns } from "../../../test/boardsFixtures";
import ProposalReview from "./ProposalReview";

const noop = (): void => {};

function proposal(
  change: MessageInitShape<typeof BoardProposalSchema>["change"],
  summary: string,
): BoardProposal {
  return create(BoardProposalSchema, {
    id: "bp-9",
    boardId: RECIPES_ID,
    summary,
    proposedBy: "bot:lead",
    decisionId: "dec-9",
    state: "open",
    change,
  });
}

function Review(props: {
  readonly proposal: BoardProposal;
  readonly canApprove?: boolean;
}): ReactElement {
  return (
    <div className="cc-reader" style={{ height: 640, padding: 24 }}>
      <ProposalReview
        found={{ proposal: props.proposal, board: boards()[1], columns: recipeColumns() }}
        lead="Team Lead"
        canApprove={props.canApprove ?? true}
        busy={false}
        error={null}
        onRule={noop}
        onChange={noop}
      />
    </div>
  );
}

/** A new column, before and after, with the rules it brings. */
export const AddColumn: Story = () => (
  <Review
    proposal={proposal(
      {
        case: "columnAdd",
        value: {
          boardId: RECIPES_ID,
          version: 9n,
          name: "Review",
          kind: ColumnKind.ACTIVE,
          after: "rb-doing",
          rules: ["link:pr", "independent"],
          movers: ["assignee"],
        },
      },
      "add column “Review” with rules link:pr, independent",
    )}
  />
);

/** New rules on QA: what each one adds or drops. */
export const ChangeRules: Story = () => (
  <Review
    proposal={proposal(
      {
        case: "columnUpdate",
        value: {
          boardId: RECIPES_ID,
          version: 9n,
          key: "rb-qa",
          setPolicy: true,
          movers: ["roles:tester"],
          rules: ["ac_all_checked", "size_at_most:M"],
          allowedFrom: [],
        },
      },
      "set rb-qa's rules to [ac_all_checked, size_at_most:M], movers [roles:tester]",
    )}
  />
);

/** Removing a column, its items moved first. */
export const RemoveColumn: Story = () => (
  <Review
    proposal={proposal(
      {
        case: "columnRemove",
        value: { boardId: RECIPES_ID, version: 9n, key: "rb-qa", moveItemsTo: "rb-doing" },
      },
      "remove column rb-qa",
    )}
  />
);

/** Seen without the approve grant: it can be read, not ruled on. */
export const ReadOnly: Story = () => (
  <Review
    canApprove={false}
    proposal={proposal(
      {
        case: "automationSet",
        value: { boardId: RECIPES_ID, version: 9n, fact: "pr.merged", columnKey: "rb-done" },
      },
      "move items to rb-done on pr.merged",
    )}
  />
);

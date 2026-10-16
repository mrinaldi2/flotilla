import type { Story } from "@ladle/react";
import { create } from "@bufbuild/protobuf";
import type { ReactElement } from "react";
import type { ProjectTab } from "../../app/selection";
import type { ItemCard } from "../../protocol/gen/flotilla/board/v1/board_pb";
import { MoveCheckSchema } from "../../protocol/gen/flotilla/board/v1/requests_pb";
import type { MoveCheck } from "../../protocol/gen/flotilla/board/v1/requests_pb";
import {
  card,
  column,
  defaultColumns,
  sampleCards,
  snapshot,
  unmet,
} from "../../test/boardFixtures";
import { FakeDaemon } from "../../test/fakeDaemon";
import { bot, project } from "../../test/fixtures";
import ProjectWindow from "../project/ProjectWindow";
import BoardView from "./BoardView";
import { columnChecks } from "./moves";
import { MoveDialog, MoveMenu, RefusalPopover } from "./MovePopovers";

const noop = (): void => {};
const noTab = (_tab: ProjectTab): void => {};
const BOTS = [
  bot({ id: "b1", name: "Desktop Dev", avatar: "color:#7aa2f7" }),
  bot({ id: "b2", name: "Architect", avatar: "color:#4ade80" }),
];

/** Guards for the stories: Ready needs refinement, Deploying is daemon-only. */
function checkFor(id: string): MoveCheck {
  return create(MoveCheckSchema, {
    itemId: id,
    columns: defaultColumns().map((c) => ({
      columnKey: c.key,
      unmet:
        c.key === "verify" || c.key === "deploying"
          ? [
              unmet(
                "move.daemon_only",
                `${c.name} is entered by the Flotilla service, not by hand.`,
              ),
            ]
          : c.key === "review"
            ? [
                unmet("review.branch", "Link a branch first.", "Link the item's branch."),
                unmet("review.change_note", "Link a change note first."),
              ]
            : [],
    })),
  });
}

function client(cards: readonly ItemCard[], canRule = false): FakeDaemon {
  return new FakeDaemon()
    .onBoard("boardWatch", () => ({ case: "board", value: { ...snapshot(cards), canRule } }))
    .onBoard("itemMoveCheck", (call) => ({
      case: "moveCheck",
      value: checkFor(call.case === "itemMoveCheck" ? (call.value.id ?? "") : ""),
    }));
}

function Window({
  cards,
  canRule = false,
}: {
  readonly cards: readonly ItemCard[];
  readonly canRule?: boolean;
}): ReactElement {
  const acme = project({ id: "story-board", name: "Flotilla" });
  return (
    <div className="main" style={{ height: 640 }}>
      <ProjectWindow project={acme} botCount={BOTS.length} tab="board" onSelectTab={noTab}>
        <BoardView
          client={client(cards, canRule)}
          project={acme}
          bots={BOTS}
          connected
          canControl
          addToast={noop}
        />
      </ProjectWindow>
    </div>
  );
}

export const Live: Story = () => <Window cards={sampleCards()} />;
export const Empty: Story = () => <Window cards={[]} />;
/** The owner on the board's home: each column's WIP limit is editable (H-099). */
export const OwnerOnHome: Story = () => <Window cards={sampleCards()} canRule />;

const columns = defaultColumns();
const h024 = card({ id: "H-024", title: "Releases tab with approval flow", columnKey: "inbox" });
const at = { x: 24, y: 24 };

export const MoveToMenu: Story = () => (
  <div className="main" style={{ height: 480 }}>
    <MoveMenu
      card={h024}
      anchor={at}
      columns={columns}
      checks={columnChecks(
        create(MoveCheckSchema, {
          itemId: "H-024",
          columns: [
            {
              columnKey: "ready",
              unmet: [
                unmet("dor.acceptance_criteria", "Needs at least one acceptance criterion."),
                unmet("dor.size", "Size is L: split it first."),
              ],
            },
            {
              columnKey: "deploying",
              unmet: [unmet("move.daemon_only", "Moves after the release ruling.")],
            },
          ],
        }),
      )}
      onChoose={noop}
      onClose={noop}
    />
  </div>
);

/**
 * A release-gated board's unlisted move (Inbox → Doing): the owner's
 * override, with a reason, rather than a plain ✓.
 */
export const MoveToMenuOverride: Story = () => (
  <div className="main" style={{ height: 480 }}>
    <MoveMenu
      card={h024}
      anchor={at}
      columns={columns}
      checks={columnChecks(
        create(MoveCheckSchema, {
          itemId: "H-024",
          columns: [
            { columnKey: "ready", unmet: [unmet("dor.size", "Size it.")] },
            {
              columnKey: "doing",
              unmet: [unmet("reason.required", "This isn't a usual move; say why.")],
              ownerOverride: true,
            },
            {
              columnKey: "review",
              unmet: [unmet("reason.required", "This isn't a usual move; say why.")],
              ownerOverride: true,
            },
            {
              columnKey: "deploying",
              unmet: [unmet("move.daemon_only", "Moves after the release ruling.")],
            },
          ],
        }),
      )}
      onChoose={noop}
      onClose={noop}
    />
  </div>
);

/** The override's reason dialog. */
export const OverrideDialog: Story = () => (
  <div className="main" style={{ height: 480 }}>
    <MoveDialog
      card={h024}
      to={column("doing")}
      needsReason
      needsOverride={false}
      override
      unmet={[unmet("reason.required", "This isn't a usual move; say why.")]}
      onConfirm={noop}
      onCancel={noop}
    />
  </div>
);

export const Refusal: Story = () => (
  <div className="main" style={{ height: 480 }}>
    <RefusalPopover
      card={h024}
      to={column("ready")}
      anchor={at}
      unmet={[
        unmet(
          "dor.acceptance_criteria",
          "Needs at least one acceptance criterion.",
          "Add one in the item.",
        ),
        unmet("dor.blocked_by", "Blocked by H-007 (Doing)."),
      ]}
      onClose={noop}
    />
  </div>
);

export const WipOverride: Story = () => (
  <div className="main" style={{ height: 480 }}>
    <MoveDialog
      card={h024}
      to={column("doing")}
      needsReason={false}
      needsOverride
      unmet={[unmet("wip.full", "Doing is at its WIP limit of 1 for Desktop Dev.")]}
      onConfirm={noop}
      onCancel={noop}
    />
  </div>
);

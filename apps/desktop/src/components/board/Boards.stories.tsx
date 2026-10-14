import type { Story } from "@ladle/react";
import { create } from "@bufbuild/protobuf";
import type { MessageInitShape } from "@bufbuild/protobuf";
import type { ReactElement } from "react";
import { ItemCardSchema, MoveBlockSchema } from "../../protocol/gen/hermes/board/v1/board_pb";
import { DaemonError } from "../../protocol/connection";
import { boardsSnapshot, RECIPES_ID } from "../../test/boardsFixtures";
import { BoardWindow, boardsClient } from "../../test/boardStory";
import NewBoardSheet from "./NewBoardSheet";
import { boards } from "../../test/boardsFixtures";

const noop = (): void => {};
const created = (): Promise<void> => Promise.resolve();

/** Recipes, a custom board: its own columns, the lead's change, two drafts. */
export const CustomBoard: Story = () => <BoardWindow openBoard={RECIPES_ID} />;

/**
 * Cards that can't move on: RB-3, which the next column's rules stop, and
 * RB-7, whose last move was refused.
 */
export const CantMove: Story = () => {
  const blocks: Readonly<Record<string, MessageInitShape<typeof MoveBlockSchema>>> = {
    "RB-3": { toColumn: "rb-doing", reason: "Nobody is assigned." },
    "RB-7": {
      toColumn: "rb-qa",
      reason: "2 acceptance criteria are not checked.",
      more: 1,
      refused: true,
    },
  };
  const cards = boardsSnapshot().cards.map((c) => {
    const block = blocks[c.id];
    return block === undefined
      ? c
      : create(ItemCardSchema, { ...c, moveBlock: create(MoveBlockSchema, block) });
  });
  return <BoardWindow openBoard={RECIPES_ID} client={boardsClient(boardsSnapshot({ cards }))} />;
};

/** Main, the release-gated board: Inbox folds into a rail. */
export const ReleaseGatedBoard: Story = () => <BoardWindow />;

/** The switcher: the boards, the default first, then archived and New board. */
export const Switcher: Story = () => (
  <BoardWindow openBoard={RECIPES_ID} initial={{ switcher: true }} />
);

/** The drafts tray: each draft with the bot it is discussed with. */
export const DraftsTray: Story = () => (
  <BoardWindow openBoard={RECIPES_ID} initial={{ drafts: true }} />
);

/** "+ New item": a title and a type, saved as a draft. */
export const NewItem: Story = () => (
  <BoardWindow openBoard={RECIPES_ID} initial={{ sheet: "item" }} />
);

/** A board without items: its columns, and how to start one. */
export const EmptyBoard: Story = () => (
  <BoardWindow
    openBoard={RECIPES_ID}
    client={boardsClient(boardsSnapshot({ cards: [], drafts: [], proposals: [] }))}
  />
);

function noBoard(owner: boolean): ReturnType<typeof boardsClient> {
  const client = boardsClient().onBoard("boardWatch", () => {
    throw new DaemonError(
      "no_board",
      "This project is linked with another computer, so its board isn't started on its own.",
    );
  });
  client.grants = owner ? ["read", "control", "approve"] : ["read"];
  return client;
}

/** A project with no board yet, for the owner. */
export const NoBoardYet: Story = () => <BoardWindow client={noBoard(true)} />;

/** No board yet, seen without the owner's grant. */
export const NoBoardYetReadOnly: Story = () => <BoardWindow client={noBoard(false)} />;

/** The board while its snapshot loads. */
export const Loading: Story = () => (
  <BoardWindow client={boardsClient().onBoard("boardWatch", () => new Promise(noop))} />
);

/** Offline: waiting for the Hermes service. */
export const Offline: Story = () => <BoardWindow connected={false} />;

function Sheet(props: { readonly name?: string; readonly keyText?: string }): ReactElement {
  return (
    <div className="main" style={{ height: 700 }}>
      <NewBoardSheet
        boards={boards()}
        initial={{ name: props.name, key: props.keyText, preset: "software" }}
        onCreate={created}
        onCancel={noop}
      />
    </div>
  );
}

/** New board: name, key suggested from it, and the presets with their columns. */
export const NewBoard: Story = () => <Sheet name="Research lab" />;

/** A key another board of the project already has is refused as you type. */
export const NewBoardKeyTaken: Story = () => <Sheet name="Recipes again" keyText="RB" />;

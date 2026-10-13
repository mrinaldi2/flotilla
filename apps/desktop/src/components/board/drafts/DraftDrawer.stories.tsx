import type { Story } from "@ladle/react";
import { create } from "@bufbuild/protobuf";
import { useRef } from "react";
import type { ReactElement } from "react";
import { MoveResultSchema } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import { recipeColumns } from "../../../test/boardsFixtures";
import { boardsClient, STORY_BOTS } from "../../../test/boardStory";
import { unmet } from "../../../test/boardFixtures";
import { DRAWER_NOW } from "../../../test/drawerFixtures";
import { useClickOnce } from "../../../test/storyClick";
import ItemDrawer from "../drawer/ItemDrawer";

const noop = (): void => {};

function Drawer(props: {
  readonly click?: string;
  readonly discuss?: string;
  readonly edit?: boolean;
  readonly client?: ReturnType<typeof boardsClient>;
}): ReactElement {
  const box = useRef<HTMLDivElement | null>(null);
  useClickOnce(box, props.click);
  const client = props.client ?? boardsClient();
  return (
    <div ref={box} className="board-view" style={{ height: 720 }}>
      <ItemDrawer
        api={client}
        client={client}
        itemId="RB-20"
        version={6n}
        columns={recipeColumns()}
        bots={STORY_BOTS}
        canComment
        leadId="lead"
        onClose={noop}
        now={() => DRAWER_NOW}
        initialDiscuss={props.discuss}
        initialEdit={props.edit}
      />
    </div>
  );
}

/** A draft: not on the board yet, its screenshot inline, attachments, the lead's changes. */
export const Draft: Story = () => <Drawer />;

/** Review: each change since you looked, with its own Undo. */
export const Review: Story = () => <Drawer click="Review" />;

/** Discuss with a bot: the lead's chat docked beside the draft. */
export const Discuss: Story = () => <Drawer discuss="lead" />;

/** The menu that picks the bot, the lead first. */
export const DiscussMenu: Story = () => <Drawer click="Discuss with a bot" />;

/** Finalize refused: the column's unmet rules, in place. */
export const FinalizeRefused: Story = () => {
  const client = boardsClient().onBoard("itemFinalize", () => ({
    case: "moved",
    value: create(MoveResultSchema, {
      outcome: {
        case: "refused",
        value: {
          unmet: [
            unmet("rule.assignee_set", "To do needs an assignee.", "Assign the item first."),
            unmet("rule.ac_present", "Add at least one acceptance criterion."),
          ],
        },
      },
    }),
  }));
  return <Drawer client={client} click="Finalize" />;
};

/** Discard asks why; the draft keeps its history. */
export const Discard: Story = () => <Drawer click="Discard" />;

/** The owner editing the draft: every field, criteria one per line. */
export const Edit: Story = () => <Drawer edit />;

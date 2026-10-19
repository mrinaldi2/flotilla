import type { Story } from "@ladle/react";
import type { ReactElement } from "react";
import { defaultColumns } from "../../../test/boardFixtures";
import { DASH_BOTS } from "../../../test/dashboardFixtures";
import { DRAWER_NOW, drawerCheck, itemDetail } from "../../../test/drawerFixtures";
import { FakeDaemon } from "../../../test/fakeDaemon";
import { bot } from "../../../test/fixtures";
import { create } from "@bufbuild/protobuf";
import { ARTIFACTS_V2 } from "../../../protocol/chat";
import { ItemAttachmentSchema } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { licensingQuestion, questionsClient, saintsQuestion } from "../../../test/questionFixtures";
import ItemDrawer from "./ItemDrawer";
import type { DrawerSize } from "./useDrawerSize";
import { DRAWER_DEFAULT } from "./useDrawerSize";

const BOTS = [...DASH_BOTS, bot({ id: "lead", name: "Team Lead" })];
const noop = (): void => {};

function Drawer(props: {
  readonly tab: "overview" | "links" | "activity";
  readonly size?: DrawerSize;
}): ReactElement {
  const client = new FakeDaemon()
    .onBoard("itemGet", () => ({ case: "item", value: itemDetail() }))
    .onBoard("itemMoveCheck", () => ({ case: "moveCheck", value: drawerCheck() }));
  return (
    <div className="board-view" style={{ height: 720 }}>
      <ItemDrawer
        api={client}
        itemId="H-017"
        columns={defaultColumns()}
        bots={BOTS}
        canComment
        onClose={noop}
        onMove={noop}
        now={() => DRAWER_NOW}
        initialTab={props.tab}
        initialSize={props.size ?? { width: DRAWER_DEFAULT, wide: false }}
      />
    </div>
  );
}

/** Where H-017 stands, its next step, and its criteria. */
export const Overview: Story = () => <Drawer tab="overview" />;
/** Its tasks, branch, change note and the item it blocks. */
export const Links: Story = () => <Drawer tab="links" />;
/** Comments and moves as one timeline, with the owner's composer. */
export const Activity: Story = () => <Drawer tab="activity" />;
/** Dragged wider from its left edge, for long specs and comments. */
export const Resized: Story = () => <Drawer tab="overview" size={{ width: 720, wide: false }} />;
/** Widened over the whole board from the header. */
export const Wide: Story = () => (
  <Drawer tab="overview" size={{ width: DRAWER_DEFAULT, wide: true }} />
);

function QuestionsDrawer(): ReactElement {
  const detail = itemDetail();
  detail.questions = [saintsQuestion({ itemId: "H-017" }), licensingQuestion()];
  detail.attachments = [
    create(ItemAttachmentSchema, {
      id: "link:H-017-change.md",
      itemId: "H-017",
      kind: "doc",
      name: "change note",
      path: "H-017-change.md",
    }),
  ];
  const client = questionsClient()
    .onBoard("itemGet", () => ({ case: "item", value: detail }))
    .onBoard("itemMoveCheck", () => ({ case: "moveCheck", value: drawerCheck() }));
  client.capabilities = [...client.capabilities, ARTIFACTS_V2];
  return (
    <div className="board-view" style={{ height: 1100 }}>
      <ItemDrawer
        api={client}
        itemId="H-017"
        columns={defaultColumns()}
        bots={BOTS}
        canComment
        canAnswer
        onClose={noop}
        onMove={noop}
        now={() => DRAWER_NOW}
      />
    </div>
  );
}

/** The card's questions above its tabs (0.22.0), and its artifact link as a document. */
export const Questions: Story = () => <QuestionsDrawer />;

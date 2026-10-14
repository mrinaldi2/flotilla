// The Board tab inside its project window, for stories of several boards
// (0.19.0): a fake Hermes service that answers the board, an item, a
// draft's changes, an attachment's bytes and the owner's thread with a bot.

import { create } from "@bufbuild/protobuf";
import type { ReactElement } from "react";
import type { ProjectTab } from "../app/selection";
import BoardView from "../components/board/BoardView";
import type { BoardProps } from "../components/board/Board";
import ProjectWindow from "../components/project/ProjectWindow";
import {
  AttachmentChunkSchema,
  MoveCheckSchema,
} from "../protocol/gen/hermes/board/v1/requests_pb";
import type { BoardSnapshot } from "../protocol/gen/hermes/board/v1/requests_pb";
import { boardsSnapshot, draftChanges, draftDetail, SCREENSHOT_PNG } from "./boardsFixtures";
import { DRAWER_NOW } from "./drawerFixtures";
import { FakeDaemon } from "./fakeDaemon";
import { bot, project } from "./fixtures";
import { ownerThreadJson } from "./homeFixtures";

export const STORY_BOTS = [
  bot({ id: "lead", name: "Team Lead", avatar: "color:#f59e0b" }),
  bot({ id: "b1", name: "Desktop Dev", avatar: "color:#7aa2f7" }),
  bot({ id: "b2", name: "Tester", avatar: "color:#4ade80" }),
];

const noop = (): void => {};
const noTab = (_tab: ProjectTab): void => {};

/** A Hermes service with a project of several boards. */
export function boardsClient(snapshot: BoardSnapshot = boardsSnapshot()): FakeDaemon {
  const fake = new FakeDaemon()
    .onBoard("boardWatch", () => ({ case: "board", value: snapshot }))
    .onBoard("boardGet", () => ({ case: "board", value: snapshot }))
    .onBoard("itemGet", () => ({ case: "item", value: draftDetail() }))
    .onBoard("itemMoveCheck", () => ({ case: "moveCheck", value: create(MoveCheckSchema, {}) }))
    .onBoard("itemDraftChanges", () => ({ case: "draftChanges", value: draftChanges() }))
    .onBoard("attachmentRead", () => ({
      case: "attachment",
      value: create(AttachmentChunkSchema, {
        attachmentId: "att-shot",
        data: SCREENSHOT_PNG,
        total: BigInt(SCREENSHOT_PNG.length),
        mime: "image/png",
      }),
    }))
    .onRequest("owner_thread_get", () => ({
      type: "owner_thread",
      req_id: "1",
      owner_thread: ownerThreadJson(),
    }));
  fake.grants = ["read", "control", "approve"];
  fake.capabilities = [...fake.capabilities, "owner_threads"];
  return fake;
}

export function BoardWindow(props: {
  readonly client?: FakeDaemon;
  readonly connected?: boolean;
  readonly openItem?: string;
  readonly openBoard?: string;
  readonly initial?: BoardProps["initial"];
  readonly height?: number;
}): ReactElement {
  const p = project({ id: "p1", name: "Recipe Box", lead_bot_id: "lead" });
  return (
    <div className="main" style={{ height: props.height ?? 700 }}>
      <ProjectWindow project={p} botCount={STORY_BOTS.length} tab="board" onSelectTab={noTab}>
        <BoardView
          client={props.client ?? boardsClient()}
          project={p}
          bots={STORY_BOTS}
          connected={props.connected ?? true}
          canControl
          addToast={noop}
          openItem={props.openItem}
          openBoard={props.openBoard}
          initial={props.initial}
          now={DRAWER_NOW}
        />
      </ProjectWindow>
    </div>
  );
}

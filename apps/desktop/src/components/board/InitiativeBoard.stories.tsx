import type { Story } from "@ladle/react";
import { useState } from "react";
import type { ReactElement } from "react";
import type { ProjectTab } from "../../app/selection";
import { DRAWER_NOW } from "../../test/drawerFixtures";
import { bot, project } from "../../test/fixtures";
import { initiativeColumns, PLAN_PROJECT, planNodes } from "../../test/planFixtures";
import { openTasks, researchClient } from "../../test/researchFixtures";
import ProjectWindow from "../project/ProjectWindow";
import type { BoardProps } from "./Board";
import BoardView from "./BoardView";
import ItemDrawer from "./drawer/ItemDrawer";

const BOTS = [
  bot({ id: "lead", name: "Game Lead", avatar: "color:#f59e0b" }),
  bot({ id: "b1", name: "Designer", avatar: "color:#f9a8d4" }),
  bot({ id: "b2", name: "Gameplay Dev", avatar: "color:#4ade80" }),
];
const noop = (): void => {};
const noTab = (_tab: ProjectTab): void => {};

function Window(props: { readonly initial?: BoardProps["initial"] }): ReactElement {
  const [client] = useState(researchClient);
  const p = project({ id: PLAN_PROJECT, name: "PhD Game", lead_bot_id: "lead" });
  return (
    <div className="main" style={{ height: 700 }}>
      <ProjectWindow project={p} botCount={BOTS.length} tab="board" plan onSelectTab={noTab}>
        <BoardView
          client={client}
          project={p}
          bots={BOTS}
          connected
          canControl
          addToast={noop}
          planNodes={planNodes()}
          initial={props.initial}
          now={DRAWER_NOW}
        />
      </ProjectWindow>
    </div>
  );
}

/** An initiative's board: the Research column, with its hint, between Ready and Doing (canvas D3). */
export const ResearchColumn: Story = () => <Window />;

/** + New item: Task, Design, Bug and Spike, and the plan node the card belongs to (canvas D4). */
export const NewItemBelongsTo: Story = () => <Window initial={{ sheet: "item" }} />;

function Drawer(props: { readonly itemId: string }): ReactElement {
  const [client] = useState(researchClient);
  return (
    <div className="board-view" style={{ height: 720 }}>
      <ItemDrawer
        api={client}
        itemId={props.itemId}
        columns={initiativeColumns()}
        bots={BOTS}
        canComment
        onClose={noop}
        onMove={noop}
        now={() => DRAWER_NOW}
        onOpenItem={noop}
        designTasks={openTasks()}
      />
    </div>
  );
}

/** A task held in Research: the start.research refusal, and the design it builds (canvas D4). */
export const TaskHeldInResearch: Story = () => <Drawer itemId="CR-28" />;

/** A done Design card, which the owner attaches to the task that builds it. */
export const DoneDesign: Story = () => <Drawer itemId="CR-19" />;

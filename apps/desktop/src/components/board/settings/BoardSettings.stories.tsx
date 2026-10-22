import type { Story } from "@ladle/react";
import { useRef } from "react";
import type { ReactElement } from "react";
import type { Board, BoardColumn } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { DaemonError } from "../../../protocol/connection";
import { useClickOnce } from "../../../test/storyClick";
import { flowColumns, flowRoles } from "../../../test/boardFlowFixtures";
import { board, boards, mainColumns, proposals, recipeColumns } from "../../../test/boardsFixtures";
import { DRAWER_NOW } from "../../../test/drawerFixtures";
import { FakeDaemon } from "../../../test/fakeDaemon";
import { bot } from "../../../test/fixtures";
import BoardSettings from "./BoardSettings";
import type { SettingsSection } from "./BoardSettings";
import type { ColumnFlow } from "./columnFlow";
import { RemoveColumnDialog } from "./ColumnsSection";

const noop = (): void => {};
const who = (actor: string): string => (actor === "bot:lead" ? "Team Lead" : "a bot");
const [main = board(), recipes = board()] = boards();

interface PanelProps {
  readonly section: SettingsSection;
  readonly board: Board;
  readonly columns: readonly BoardColumn[];
  readonly canEdit: boolean;
  readonly expanded?: string;
  readonly api: FakeDaemon;
  readonly flow?: ColumnFlow;
}

function Panel(props: PanelProps): ReactElement {
  return (
    <div className="board-view" style={{ height: 700 }}>
      <BoardSettings
        api={props.api}
        board={props.board}
        columns={props.columns}
        counts={
          new Map([
            ["rb-todo", 2],
            ["rb-qa", 1],
          ])
        }
        canEdit={props.canEdit}
        proposals={proposals()}
        who={who}
        adopt={noop}
        onClose={noop}
        initialSection={props.section}
        initiallyExpanded={props.expanded}
        now={DRAWER_NOW}
        flow={props.flow}
      />
    </div>
  );
}

/** Recipes, a custom board, for the owner. */
function Recipes(props: Partial<PanelProps> & { readonly section: SettingsSection }): ReactElement {
  return (
    <Panel board={recipes} columns={recipeColumns()} canEdit api={new FakeDaemon()} {...props} />
  );
}

/** General: name, the fixed key, description, structure policy, default, archive. */
export const General: Story = () => <Recipes section="general" />;

/** Columns: kind chips, WIP, visibility, order, and each column's rules in words. */
export const Columns: Story = () => <Recipes section="columns" />;

/** The bots of the On entry story: two Developers, a Tester asleep. */
const FLOW: ColumnFlow = {
  roles: flowRoles(),
  bots: [
    bot({ id: "b1", name: "Desktop Dev" }),
    bot({ id: "b2", name: "Tester", state: "asleep" }),
    bot({ id: "b3", name: "Desktop Dev 2" }),
  ],
  onCreateBot: noop,
  creating: null,
};

/**
 * The board runs the SDLC (0.24.0): each column's On entry role and who
 * holds it; nobody holds Product, so To do offers a Product bot. Then the
 * chain a card follows.
 */
export const ColumnsOnEntry: Story = () => (
  <Recipes section="columns" columns={flowColumns()} flow={FLOW} />
);

/** A column expanded: who moves items in, where from, and the rule checklist. */
export const ColumnRules: Story = () => <Recipes section="columns" expanded="rb-qa" />;

/** Automations: one row per fact the modules report. */
export const Automations: Story = () => <Recipes section="automations" />;

/** Item types: built in for now, said in words. */
export const ItemTypes: Story = () => <Recipes section="types" />;

/** A release-gated board: renames, order, WIP and visibility only. */
export const ReleaseGated: Story = () => (
  <Recipes section="columns" board={main} columns={mainColumns()} />
);

/** Without the approve grant, or off the board's home: read only. */
export const ReadOnly: Story = () => <Recipes section="columns" canEdit={false} />;

/** Removing a column that holds items asks where they go. */
export const RemoveColumn: Story = () => {
  const columns = recipeColumns();
  const qa = columns.find((c) => c.key === "rb-qa");
  return qa === undefined ? (
    <p />
  ) : (
    <div className="main" style={{ height: 480 }}>
      <RemoveColumnDialog
        column={qa}
        items={3}
        others={columns.filter((c) => c.key !== "rb-qa")}
        onRemove={noop}
        onCancel={noop}
      />
    </div>
  );
};

/** A change the Flotilla service refuses: its words at the foot of the panel. */
export const Refused: Story = () => {
  const box = useRef<HTMLDivElement | null>(null);
  useClickOnce(box, "Make this the default board");
  const api = new FakeDaemon().onBoard("boardUpdate", () => {
    throw new DaemonError("conflict", "the board changed; re-read it");
  });
  return (
    <div ref={box}>
      <Recipes section="general" api={api} />
    </div>
  );
};

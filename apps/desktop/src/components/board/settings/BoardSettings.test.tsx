import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Role } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { BoardColumn } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import { StructureResultSchema } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import type { BoardCall } from "../../../protocol/board";
import { flowColumns, flowRoles } from "../../../test/boardFlowFixtures";
import { board, boardsSnapshot, proposals, recipeColumns } from "../../../test/boardsFixtures";
import { FakeDaemon } from "../../../test/fakeDaemon";
import { bot } from "../../../test/fixtures";
import BoardSettings from "./BoardSettings";
import type { SettingsSection } from "./BoardSettings";
import { afterFor, limitChange, policyLine } from "./columnEdits";
import type { ColumnFlow } from "./columnFlow";
import { heldBy, namesText } from "./columnFlow";

const recipes = board({
  id: "b-recipes",
  name: "Recipes",
  key: "RB",
  flow: "custom",
  isDefault: false,
  version: 9n,
  automations: { "pr.merged": "rb-done" },
});

function renderSettings(
  section: SettingsSection,
  over: {
    gated?: boolean;
    canEdit?: boolean;
    flow?: ColumnFlow;
    columns?: readonly BoardColumn[];
  } = {},
) {
  const applied = create(StructureResultSchema, {
    boardId: recipes.id,
    outcome: { case: "applied", value: boardsSnapshot() },
  });
  const fake = new FakeDaemon();
  for (const arm of [
    "boardUpdate",
    "columnUpdate",
    "columnAdd",
    "columnRemove",
    "automationSet",
  ] as const) {
    fake.onBoard(arm, () => ({ case: "structure", value: applied }));
  }
  const adopt = vi.fn<(snapshot: unknown) => void>();
  render(
    <BoardSettings
      api={fake}
      board={over.gated === true ? board() : recipes}
      columns={over.columns ?? recipeColumns()}
      counts={new Map([["rb-qa", 2]])}
      canEdit={over.canEdit ?? true}
      proposals={proposals()}
      who={() => "Team Lead"}
      adopt={adopt}
      onClose={vi.fn<() => void>()}
      initialSection={section}
      now={Date.parse("2026-10-05T17:00:00Z")}
      flow={over.flow}
    />,
  );
  const last = (): BoardCall | undefined => fake.boardCalls.at(-1);
  return { fake, adopt, last };
}

describe("a column row's arithmetic", () => {
  const cols = recipeColumns();
  const [todo, doing, qa] = cols;

  it("moves a column one place either way", () => {
    expect(todo && afterFor(cols, todo, -1)).toBeNull();
    expect(doing && afterFor(cols, doing, -1)).toBe("");
    expect(qa && afterFor(cols, qa, -1)).toBe("rb-todo");
    expect(qa && afterFor(cols, qa, 1)).toBe("rb-done");
    const last = cols.at(-1);
    expect(last && afterFor(cols, last, 1)).toBeNull();
  });

  it("turns a typed WIP limit into a change", () => {
    expect(limitChange("", 3)).toEqual({ clearWipLimit: true });
    expect(limitChange(" ", undefined)).toBeNull();
    expect(limitChange("4", 3)).toEqual({ wipLimit: 4 });
    expect(limitChange("3", 3)).toBeNull();
    expect(limitChange("0", 3)).toBeNull();
    expect(limitChange("2.5", undefined)).toBeNull();
  });

  it("words a column's policy", () => {
    expect(qa && policyLine(qa, cols, true)).toBe(
      "Roles tester · All criteria checked · Has a link of kind pr · from Doing",
    );
    expect(qa && policyLine(qa, cols, false)).toBe("Built-in release-gated rules");
  });
});

/** The panel's foot once a save has answered. */
const idle = (): Promise<HTMLElement> => screen.findByText(/^Last change/);

describe("board settings", () => {
  it("saves the name and description, the policy, the default and archiving", async () => {
    const user = userEvent.setup();
    const { last, adopt } = renderSettings("general");
    const name = screen.getByRole("textbox", { name: "Name" });
    await user.clear(name);
    await user.type(name, "Kitchen");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(last()).toMatchObject({
      case: "boardUpdate",
      value: { boardId: "b-recipes", version: 9n, name: "Kitchen" },
    });
    expect(adopt).toHaveBeenCalled();
    await user.click(screen.getByRole("radio", { name: /Lead decides/ }));
    expect(last()).toMatchObject({ value: { structurePolicy: "lead_free" } });
    await user.click(screen.getByRole("button", { name: "Make this the default board" }));
    expect(last()).toMatchObject({ value: { isDefault: true } });
    await user.click(screen.getByRole("button", { name: "Archive this board" }));
    expect(last()).toMatchObject({ value: { archived: true } });
  });

  it("renames a column once on Enter, though focus then leaves it, and keeps its focus", async () => {
    const user = userEvent.setup();
    const { fake } = renderSettings("columns");
    const name = screen.getByRole("textbox", { name: "Name of QA" });
    await user.clear(name);
    await user.type(name, "Testing{Enter}");
    expect(name).toHaveFocus();
    await user.tab();
    await idle();
    const renames = fake.boardCalls.filter((c) => c.case === "columnUpdate");
    expect(renames).toHaveLength(1);
    expect(renames[0]).toMatchObject({ value: { key: "rb-qa", name: "Testing" } });
  });

  it("names a column's visibility box for the column, whatever its state", () => {
    renderSettings("columns");
    expect(screen.getByRole("checkbox", { name: "Show QA" })).toBeChecked();
  });

  it("changes a column's kind, limit, visibility and place, and adds one", async () => {
    const user = userEvent.setup();
    const { last } = renderSettings("columns");
    await user.selectOptions(screen.getByRole("combobox", { name: "Kind of QA" }), "3");
    await idle();
    expect(last()).toMatchObject({ case: "columnUpdate", value: { key: "rb-qa", kind: 3 } });
    const wip = screen.getByRole("spinbutton", { name: "WIP limit of QA" });
    await user.type(wip, "2{Enter}");
    await idle();
    expect(last()).toMatchObject({ value: { key: "rb-qa", wipLimit: 2 } });
    await user.click(screen.getByRole("checkbox", { name: "Show QA" }));
    await idle();
    expect(last()).toMatchObject({ value: { key: "rb-qa", visible: false } });
    await user.click(screen.getByRole("button", { name: "Move QA left" }));
    await idle();
    expect(last()).toMatchObject({ value: { key: "rb-qa", after: "rb-todo" } });
    await user.type(screen.getByRole("textbox", { name: "New column name" }), "Review");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await idle();
    expect(last()).toMatchObject({ case: "columnAdd", value: { name: "Review", kind: 2 } });
  });

  it("edits a column's rules as one change", async () => {
    const user = userEvent.setup();
    const { last } = renderSettings("columns");
    await user.click(screen.getByRole("button", { name: "Show the rules of QA" }));
    await user.click(screen.getByRole("checkbox", { name: "The lead" }));
    await user.click(screen.getByRole("checkbox", { name: "Size at most" }));
    await user.selectOptions(screen.getByRole("combobox", { name: "QA: Size at most" }), "L");
    await user.click(screen.getByRole("checkbox", { name: "Template sections filled" }));
    await user.type(screen.getByRole("textbox", { name: "QA: Template sections filled" }), "Goal");
    await user.click(screen.getByRole("checkbox", { name: "dev" }));
    await user.click(screen.getByRole("checkbox", { name: "Done" }));
    await user.click(screen.getByRole("button", { name: "Save rules" }));
    expect(last()).toMatchObject({
      case: "columnUpdate",
      value: {
        key: "rb-qa",
        setPolicy: true,
        movers: ["lead", "roles:tester,dev"],
        rules: ["ac_all_checked", "link:pr", "size_at_most:L", "fields_filled:Goal"],
        allowedFrom: ["rb-doing", "rb-done"],
      },
    });
    await user.click(screen.getByRole("checkbox", { name: "The assignee" }));
    await user.click(screen.getByRole("button", { name: "Reset" }));
    expect(screen.getByRole("checkbox", { name: "The assignee" })).not.toBeChecked();
  });

  it("asks where a column's items go before removing it", async () => {
    const user = userEvent.setup();
    const { last } = renderSettings("columns");
    await user.click(screen.getByRole("button", { name: "Remove QA" }));
    const dialog = screen.getByRole("dialog", { name: "Remove QA" });
    expect(dialog).toHaveTextContent("It holds 2 items.");
    await user.selectOptions(within(dialog).getByRole("combobox"), "rb-done");
    await user.click(within(dialog).getByRole("button", { name: "Remove column" }));
    expect(last()).toMatchObject({
      case: "columnRemove",
      value: { key: "rb-qa", moveItemsTo: "rb-done" },
    });
    await user.click(screen.getByRole("button", { name: "Remove To do" }));
    const empty = screen.getByRole("dialog", { name: "Remove To do" });
    expect(empty).toHaveTextContent("It holds no items.");
    await user.click(within(empty).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("maps a fact to a column, or to none", async () => {
    const user = userEvent.setup();
    const { last } = renderSettings("automations");
    const merged = screen.getByRole("combobox", { name: "When PR merged, move to" });
    expect(merged).toHaveValue("rb-done");
    await user.selectOptions(merged, "");
    expect(last()).toMatchObject({
      case: "automationSet",
      value: { fact: "pr.merged", columnKey: undefined },
    });
  });

  it("says why a release-gated board's rules and automations stay, and shows read only", () => {
    renderSettings("automations", { gated: true, canEdit: false });
    expect(screen.getByText(/Automations are for custom boards/)).toBeInTheDocument();
    expect(screen.getByRole("note")).toHaveTextContent("Read only");
  });
});

const FLOW_BOTS = [
  bot({ id: "b1", name: "Desktop Dev" }),
  bot({ id: "b2", name: "Tester", state: "asleep" }),
  bot({ id: "b3", name: "Desktop Dev 2", state: "asleep" }),
];

function flowOf(over: Partial<ColumnFlow> = {}): ColumnFlow {
  return { roles: flowRoles(), bots: FLOW_BOTS, creating: null, ...over };
}

describe("a column's On entry role", () => {
  it("words who holds a role", () => {
    const flow = flowOf();
    expect(heldBy(Role.UNSPECIFIED, flow)).toEqual({ text: "—", unheld: false });
    expect(heldBy(Role.PRODUCT, flow)).toEqual({ text: "nobody yet", unheld: true });
    expect(heldBy(Role.TESTER, flow).text).toBe("Tester · asleep");
    expect(heldBy(Role.DEV, flow).text).toBe("Desktop Dev, Desktop Dev 2");
    expect(heldBy(Role.LEAD, flow).text).toBe("a bot");
    expect(namesText([])).toBe("");
    expect(namesText(["To do", "Inbox", "Ready"])).toBe("To do, Inbox and Ready");
  });

  it("saves the role a column assigns, and clears it with Nobody", async () => {
    const user = userEvent.setup();
    const { last } = renderSettings("columns", { flow: flowOf(), columns: flowColumns() });
    const qa = screen.getByRole("combobox", { name: "On entry, QA assigns" });
    expect(qa).toHaveValue(String(Role.TESTER));
    await user.selectOptions(qa, "Architect");
    await idle();
    expect(last()).toMatchObject({
      case: "columnUpdate",
      value: { key: "rb-qa", onEntry: Role.ARCHITECT },
    });
    expect(last()?.value).not.toHaveProperty("setPolicy");
    // A card never goes to the lead or the coach.
    const roles = within(qa)
      .getAllByRole("option")
      .map((o) => o.textContent);
    expect(roles).toContain("Product");
    expect(roles).not.toContain("Lead");
    expect(roles).not.toContain("Coach");
    await user.selectOptions(qa, "Nobody");
    await idle();
    expect(last()).toMatchObject({ value: { key: "rb-qa", onEntry: Role.UNSPECIFIED } });
  });

  it("shows who holds each role, and offers a bot for the one nobody holds", async () => {
    const user = userEvent.setup();
    const onCreateBot = vi.fn<(role: Role) => void>();
    renderSettings("columns", { flow: flowOf({ onCreateBot }), columns: flowColumns() });
    expect(screen.getByText("nobody yet")).toHaveClass("column-flow-unheld");
    expect(screen.getByText("Tester · asleep")).toBeInTheDocument();
    expect(screen.getByText("Nobody holds Product")).toBeInTheDocument();
    expect(
      screen.getByText("Cards entering To do stay with whoever moved them."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Create a Product bot" }));
    expect(onCreateBot).toHaveBeenCalledWith(Role.PRODUCT);
    const chain = screen.getByRole("region", { name: "How a card flows" });
    expect(
      within(chain)
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["To do · Product", "→ Doing · Developer", "→ QA · Tester", "→ Done"]);
    expect(chain).toHaveTextContent(
      "A failed review moves it back to Doing and re-assigns the Developer, with the Tester's note.",
    );
  });

  it("says a bot is being created", () => {
    renderSettings("columns", {
      flow: flowOf({ onCreateBot: vi.fn<(role: Role) => void>(), creating: Role.PRODUCT }),
      columns: flowColumns(),
    });
    expect(screen.getByRole("button", { name: "Creating…" })).toBeDisabled();
  });

  it("works on a release-gated board, read only for anyone but the owner", () => {
    renderSettings("columns", {
      gated: true,
      canEdit: false,
      flow: flowOf({ onCreateBot: vi.fn<(role: Role) => void>() }),
      columns: flowColumns(),
    });
    expect(screen.getByRole("combobox", { name: "On entry, QA assigns" })).toBeDisabled();
    expect(screen.getByText("Nobody holds Product")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Create a/ })).toBeNull();
  });

  it("isn't offered by a service before 0.24, or with no column assigning a role", () => {
    renderSettings("columns", { columns: flowColumns() });
    expect(screen.queryByRole("combobox", { name: /On entry/ })).toBeNull();
    renderSettings("columns", { flow: flowOf() });
    expect(screen.queryByRole("region", { name: "How a card flows" })).toBeNull();
  });
});

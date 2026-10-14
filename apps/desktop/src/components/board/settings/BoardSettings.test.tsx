import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { StructureResultSchema } from "../../../protocol/gen/hermes/board/v1/requests_pb";
import type { BoardCall } from "../../../protocol/board";
import { board, boardsSnapshot, proposals, recipeColumns } from "../../../test/boardsFixtures";
import { FakeDaemon } from "../../../test/fakeDaemon";
import BoardSettings from "./BoardSettings";
import type { SettingsSection } from "./BoardSettings";
import { afterFor, limitChange, policyLine } from "./columnEdits";

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
  over: { gated?: boolean; canEdit?: boolean } = {},
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
      columns={recipeColumns()}
      counts={new Map([["rb-qa", 2]])}
      canEdit={over.canEdit ?? true}
      proposals={proposals()}
      who={() => "Team Lead"}
      adopt={adopt}
      onClose={vi.fn<() => void>()}
      initialSection={section}
      now={Date.parse("2026-10-05T17:00:00Z")}
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

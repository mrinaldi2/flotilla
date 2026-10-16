import { create } from "@bufbuild/protobuf";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { DaemonError } from "../../protocol/connection";
import {
  EditResultSchema,
  StructureResultSchema,
} from "../../protocol/gen/flotilla/board/v1/requests_pb";
import { board, boardsSnapshot, draftItem, RECIPES_ID } from "../../test/boardsFixtures";
import { BoardWindow, boardsClient } from "../../test/boardStory";
import { loadBoardPrefs } from "./boardPrefs";

function columnNames(): string[] {
  return screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent ?? "");
}

describe("a project with several boards", () => {
  it("switches boards from the toolbar and remembers the choice", async () => {
    const user = userEvent.setup();
    render(<BoardWindow openBoard={RECIPES_ID} />);
    await screen.findByText("Shopping list by aisle");
    expect(columnNames()).toEqual(["To do", "Doing", "QA", "Done"]);
    await user.click(screen.getByRole("button", { name: "Board: Recipes. Switch board" }));
    const menu = screen.getByRole("menu", { name: "Boards" });
    expect(within(menu).getByRole("menuitemradio", { name: /Recipes/ })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await user.click(within(menu).getByRole("menuitemradio", { name: /Main/ }));
    expect(columnNames()).toContain("Awaiting owner");
    expect(loadBoardPrefs("p1").board).toBe("p1/main");
  });

  it("starts a new item as a draft on the board shown and opens it", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("itemCreate", () => ({
      case: "edited",
      value: create(EditResultSchema, {
        outcome: { case: "done", value: draftItem() },
      }),
    }));
    render(<BoardWindow client={client} openBoard={RECIPES_ID} />);
    await screen.findByText("Shopping list by aisle");
    await user.click(screen.getByRole("button", { name: "+ New item" }));
    await user.type(screen.getByRole("textbox", { name: "Title" }), "Unit conversion");
    await user.click(screen.getByRole("button", { name: "Save as draft" }));
    const created = client.boardCalls.find((c) => c.case === "itemCreate");
    expect(created?.case === "itemCreate" && created.value).toMatchObject({
      title: "Unit conversion",
      boardId: RECIPES_ID,
      draft: true,
    });
    expect(await screen.findByRole("complementary", { name: "Draft RB-20" })).toBeInTheDocument();
    expect(screen.getByText("not on the board yet", { exact: false })).toBeInTheDocument();
  });

  it("opens a draft from the tray, with the bot it is discussed with", async () => {
    const user = userEvent.setup();
    render(<BoardWindow openBoard={RECIPES_ID} />);
    await user.click(await screen.findByRole("button", { name: /2 drafts/ }));
    // Its type in a glyph and a word, never the glyph alone.
    const entry = await screen.findByRole("menuitem", { name: /Feature · RB-20/ });
    expect(await within(entry).findByText("Discussing with Team Lead")).toBeInTheDocument();
    await user.click(entry);
    expect(await screen.findByRole("complementary", { name: "Draft RB-20" })).toBeInTheDocument();
  });

  it("renames a column from the settings, with the board version read", async () => {
    const user = userEvent.setup();
    const client = boardsClient().onBoard("columnUpdate", () => {
      throw new DaemonError("conflict", "the board changed; re-read it");
    });
    render(<BoardWindow client={client} openBoard={RECIPES_ID} />);
    await screen.findByText("Shopping list by aisle");
    await user.click(screen.getByRole("button", { name: "Board" }));
    await user.click(screen.getByRole("tab", { name: "Columns" }));
    const name = screen.getByRole("textbox", { name: "Name of QA" });
    await user.clear(name);
    await user.type(name, "Testing{Enter}");
    const update = client.boardCalls.find((c) => c.case === "columnUpdate");
    expect(update?.case === "columnUpdate" && update.value).toMatchObject({
      boardId: RECIPES_ID,
      version: 9n,
      key: "rb-qa",
      name: "Testing",
    });
    expect(await screen.findByRole("alert")).toHaveTextContent("the board changed; re-read it");
  });

  it("creates a board from a preset and shows it", async () => {
    const user = userEvent.setup();
    const after = boardsSnapshot();
    after.boards.push(
      board({ id: "b-lab", name: "Lab", key: "L", isDefault: false, flow: "custom" }),
    );
    const client = boardsClient().onBoard("boardCreate", () => ({
      case: "structure",
      value: create(StructureResultSchema, {
        boardId: "b-lab",
        outcome: { case: "applied", value: after },
      }),
    }));
    render(<BoardWindow client={client} openBoard={RECIPES_ID} />);
    await user.click(await screen.findByRole("button", { name: "Board: Recipes. Switch board" }));
    await user.click(screen.getByRole("menuitem", { name: "New board…" }));
    await user.type(screen.getByRole("textbox", { name: "Name" }), "Lab");
    expect(screen.getByRole("textbox", { name: "Key" })).toHaveValue("L");
    await user.click(screen.getByRole("radio", { name: /Software/ }));
    await user.click(screen.getByRole("button", { name: "Create board" }));
    const call = client.boardCalls.find((c) => c.case === "boardCreate");
    expect(call?.case === "boardCreate" && call.value).toMatchObject({
      name: "Lab",
      key: "L",
      preset: "software",
    });
    expect(
      await screen.findByRole("button", { name: "Board: Lab. Switch board" }),
    ).toBeInTheDocument();
  });

  it("shows the lead's change until it is dismissed", async () => {
    const user = userEvent.setup();
    render(<BoardWindow openBoard={RECIPES_ID} />);
    const notice = await screen.findByText(/Team Lead: add column “QA”/);
    expect(notice).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Dismiss the notice" }));
    expect(screen.queryByText(/Team Lead: add column “QA”/)).toBeNull();
  });
});

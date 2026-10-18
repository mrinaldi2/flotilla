import { create } from "@bufbuild/protobuf";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { ARTIFACTS_V2 } from "../../../protocol/chat";
import { ItemAttachmentSchema } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { ItemAttachment } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import type { ItemDetail } from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { defaultColumns } from "../../../test/boardFixtures";
import { DASH_BOTS } from "../../../test/dashboardFixtures";
import { DRAWER_NOW, drawerCheck, itemDetail } from "../../../test/drawerFixtures";
import type { FakeDaemon } from "../../../test/fakeDaemon";
import {
  calendarQuestion,
  licensingQuestion,
  questionsClient,
  saintsQuestion,
} from "../../../test/questionFixtures";
import ItemDrawer from "./ItemDrawer";

function withQuestions(): ItemDetail {
  const detail = itemDetail();
  detail.questions = [licensingQuestion(), saintsQuestion({ itemId: "H-017" })];
  return detail;
}

function setup(fake: FakeDaemon, detail: ItemDetail, canAnswer = true): void {
  fake
    .onBoard("itemGet", () => ({ case: "item", value: detail }))
    .onBoard("itemMoveCheck", () => ({ case: "moveCheck", value: drawerCheck() }));
  render(
    <ItemDrawer
      api={fake}
      itemId="H-017"
      columns={defaultColumns()}
      bots={DASH_BOTS}
      canComment
      canAnswer={canAnswer}
      onClose={() => {}}
      now={() => DRAWER_NOW}
    />,
  );
}

describe("ItemDrawer questions", () => {
  it("lists the card's questions above the tabs, open ones first, and answers one", async () => {
    const user = userEvent.setup();
    const fake = questionsClient([saintsQuestion({ itemId: "H-017" })]);
    setup(fake, withQuestions());
    const section = await screen.findByRole("region", { name: "Questions" });
    expect(within(section).getByRole("heading")).toHaveTextContent(
      "Questions · 1 open · 1 holds the gate",
    );
    const cards = within(section).getAllByRole("listitem");
    expect(cards[0]).toHaveAccessibleName(/celebrated saints/);
    expect(cards[1]).toHaveAccessibleName(/LEV and the CEI/);
    await user.click(within(section).getByRole("button", { name: /celebrated saints/ }));
    await user.click(within(section).getByRole("button", { name: "Answer" }));
    await waitFor(() =>
      expect(within(section).getByRole("heading")).toHaveTextContent(/^Questions$/),
    );
    expect(within(section).queryByRole("button", { name: "Answer" })).toBeNull();
  });

  it("takes in a question asked on this card, not another's", async () => {
    const fake = questionsClient();
    setup(fake, withQuestions());
    await screen.findByRole("region", { name: "Questions" });
    fake.emitQuestionChange({
      $typeName: "flotilla.board.v1.QuestionChange",
      projectId: "p1",
      question: calendarQuestion(),
    });
    fake.emitQuestionChange({
      $typeName: "flotilla.board.v1.QuestionChange",
      projectId: "p1",
      question: { ...calendarQuestion(), id: "d-new", itemId: "H-017", text: "A new one?" },
    });
    const section = screen.getByRole("region", { name: "Questions" });
    expect(await within(section).findByText("A new one?")).toBeInTheDocument();
    expect(within(section).queryByText(/national calendar/)).toBeNull();
  });

  it("lets a connection without the approve grant read them only", async () => {
    setup(questionsClient(), withQuestions(), false);
    const section = await screen.findByRole("region", { name: "Questions" });
    expect(within(section).queryByRole("button", { name: "Answer" })).toBeNull();
  });

  it("shows none with a daemon before questions_v1", async () => {
    const fake = questionsClient();
    fake.capabilities = [];
    setup(fake, withQuestions());
    await screen.findByRole("heading", { name: "Backlog and meetings data model in the DB" });
    expect(screen.queryByRole("region", { name: "Questions" })).toBeNull();
  });
});

function doc(id: string, name: string, path: string): ItemAttachment {
  return create(ItemAttachmentSchema, { id, itemId: "H-017", kind: "doc", name, path });
}

/** A change note attached, and linked as an artifact too, and a spec. */
function withDocs(): ItemDetail {
  const detail = itemDetail();
  detail.attachments = [
    doc("a1", "change note", "H-017-change.md"),
    doc("link:H-017-change.md", "change note", "H-017-change.md"),
    doc("a2", "spec", "specs/items.md"),
  ];
  return detail;
}

describe("ItemDrawer documents", () => {
  it("lists attachments and artifact links as one Documents list with artifacts_v2", async () => {
    const user = userEvent.setup();
    const fake = questionsClient();
    fake.capabilities = [ARTIFACTS_V2];
    setup(fake, withDocs());
    const docs = await screen.findByRole("region", { name: "Documents" });
    expect(within(docs).getByRole("heading")).toHaveTextContent("Documents 2");
    await user.click(screen.getByRole("tab", { name: "Links 3" }));
    expect(screen.getByRole("tabpanel")).not.toHaveTextContent("Artifacts");
  });

  it("keeps Attachments and the Links tab's artifacts with an older daemon", async () => {
    const user = userEvent.setup();
    const fake = questionsClient();
    fake.capabilities = [];
    setup(fake, withDocs());
    const docs = await screen.findByRole("region", { name: "Attachments" });
    expect(within(docs).getByRole("heading")).toHaveTextContent("Attachments 3");
    await user.click(screen.getByRole("tab", { name: "Links 4" }));
    expect(screen.getByRole("tabpanel")).toHaveTextContent("Artifacts");
  });
});

import { create } from "@bufbuild/protobuf";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { Artifact, ArtifactGroup } from "../../protocol/chat";
import { ARTIFACTS_V2 } from "../../protocol/chat";
import { DaemonError } from "../../protocol/connection";
import { ItemDetailSchema } from "../../protocol/gen/flotilla/board/v1/requests_pb";
import type { RequestBody } from "../../protocol/requests";
import { FakeDaemon } from "../../test/fakeDaemon";
import * as fx from "../../test/fixtures";
import { GROUPED_ARTIFACTS } from "../../test/artifactFixtures";
import FilesPanel from "./FilesPanel";

type ListBody = Extract<RequestBody, { readonly type: "list_artifacts" }>;

function lists(fake: FakeDaemon): ListBody[] {
  return fake.requests.flatMap((r) => (r.body.type === "list_artifacts" ? [r.body] : []));
}

function grouped(files: readonly Artifact[] = GROUPED_ARTIFACTS): FakeDaemon {
  const fake = new FakeDaemon().onRequest("list_artifacts", (body) => {
    const group: ArtifactGroup | undefined =
      body.type === "list_artifacts" ? body.group : undefined;
    return {
      type: "artifacts",
      req_id: "1",
      project_id: "p1",
      artifacts: files.filter((f) => f.group === (group ?? "shared")),
      has_more: false,
      next_before: null,
    };
  });
  fake.capabilities = [...fake.capabilities, ARTIFACTS_V2];
  return fake;
}

function show(fake: FakeDaemon, canControl = true): void {
  render(
    <FilesPanel
      client={fake}
      bot={fx.bot()}
      connected
      selected={null}
      onSelect={() => {}}
      canControl={canControl}
    />,
  );
}

describe("artifacts by kind", () => {
  it("lists the shared files first, with the cards that use each", async () => {
    const fake = grouped();
    show(fake);
    expect(await screen.findByText("Saints dataset")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Shared" })).toHaveAttribute("aria-selected", "true");
    const row = screen.getByText("Saints dataset").closest("li");
    expect(row).toHaveTextContent("on");
    expect(within(row ?? document.body).getByText("ASL-4")).toBeInTheDocument();
    expect(within(row ?? document.body).getByText("ASL-7")).toBeInTheDocument();
    expect(lists(fake)[0]).toEqual({
      type: "list_artifacts",
      project_id: "p1",
      group: "shared",
      limit: 100,
      before: undefined,
    });
  });

  it("reads the uploads and the checks on their own", async () => {
    const user = userEvent.setup();
    const fake = grouped();
    show(fake);
    await screen.findByText("Saints dataset");
    await user.click(screen.getByRole("tab", { name: "Uploads" }));
    expect(await screen.findByText("calendar-2026.pdf")).toBeInTheDocument();
    expect(screen.queryByText("Saints dataset")).toBeNull();
    await user.click(screen.getByRole("tab", { name: "Checks" }));
    expect(await screen.findByText("9b2f0c1a4e5d-build.log")).toBeInTheDocument();
    expect(lists(fake).map((b) => b.group)).toEqual(["shared", "uploads", "checks"]);
  });

  it("says when a group is empty", async () => {
    const user = userEvent.setup();
    show(grouped([]));
    expect(await screen.findByText("No shared files in this project yet.")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: "Checks" }));
    expect(await screen.findByText("No check logs yet.")).toBeInTheDocument();
  });

  it("reads older pages from next_before, and starts over on Refresh", async () => {
    const user = userEvent.setup();
    const [first, second] = GROUPED_ARTIFACTS;
    const fake = new FakeDaemon().onRequest("list_artifacts", (body) => {
      const before = body.type === "list_artifacts" ? body.before : undefined;
      return {
        type: "artifacts",
        req_id: "1",
        project_id: "p1",
        artifacts: before === undefined ? [first ?? GROUPED_ARTIFACTS[0]] : [second ?? first],
        has_more: before === undefined,
        next_before: before === undefined ? "2026-10-18T08:00:00Z|a.md" : null,
      };
    });
    show(fake);
    await user.click(await screen.findByRole("button", { name: "Show older" }));
    await waitFor(() => expect(screen.getAllByRole("listitem")).toHaveLength(2));
    expect(screen.queryByRole("button", { name: "Show older" })).toBeNull();
    expect(lists(fake)[1]).toMatchObject({ limit: 100, before: "2026-10-18T08:00:00Z|a.md" });
    // An older daemon: no groups, no group asked.
    expect(screen.queryByRole("tab")).toBeNull();
    expect(lists(fake)[0]?.group).toBeUndefined();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await waitFor(() => expect(lists(fake)).toHaveLength(3));
    expect(lists(fake)[2]?.before).toBeUndefined();
  });

  it("says why the list couldn't be read", async () => {
    const fake = new FakeDaemon().onRequest("list_artifacts", () => {
      throw new DaemonError("forbidden", "not yours");
    });
    show(fake);
    expect(await screen.findByText("not yours")).toBeInTheDocument();
  });
});

describe("tidy up", () => {
  it("lists the shared files no card uses, and keeps one on this computer", async () => {
    const user = userEvent.setup();
    show(grouped());
    await user.click(await screen.findByRole("tab", { name: "Tidy up" }));
    const loose = await screen.findByText("notes-0917.md");
    expect(screen.queryByText("Saints dataset")).toBeNull();
    expect(screen.getByText("frames-run3.csv")).toBeInTheDocument();
    const row = loose.closest("li");
    await user.click(within(row ?? document.body).getByRole("button", { name: "Keep" }));
    expect(screen.getByRole("status")).toHaveTextContent("Kept notes-0917.md.");
    expect(screen.queryByText("notes-0917.md")).toBeNull();
    expect(localStorage.getItem("flotilla.artifacts.kept.p1")).toBe('["notes-0917.md"]');
  });

  it("attaches a file to a card, then reads the list again", async () => {
    const user = userEvent.setup();
    const fake = grouped().onBoard("itemAttach", () => ({
      case: "item",
      value: create(ItemDetailSchema, {}),
    }));
    show(fake);
    await user.click(await screen.findByRole("tab", { name: "Tidy up" }));
    const row = (await screen.findByText("frames-run3.csv")).closest("li");
    const inRow = within(row ?? document.body);
    await user.click(inRow.getByRole("button", { name: "Attach to a card…" }));
    await user.click(inRow.getByRole("button", { name: "Cancel" }));
    await user.click(inRow.getByRole("button", { name: "Attach to a card…" }));
    await user.type(inRow.getByRole("textbox", { name: /Card to attach/ }), " ASL-9 ");
    await user.click(inRow.getByRole("button", { name: "Attach" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Attached frames-run3.csv to ASL-9.",
    );
    expect(fake.boardCalls).toContainEqual({
      case: "itemAttach",
      value: { id: "ASL-9", artifact: "runs/frames-run3.csv" },
    });
    await waitFor(() => expect(lists(fake).length).toBeGreaterThan(1));
  });

  it("says why an attach was refused", async () => {
    const user = userEvent.setup();
    const fake = grouped().onBoard("itemAttach", () => {
      throw new DaemonError("not_found", "no card ASL-99");
    });
    show(fake);
    await user.click(await screen.findByRole("tab", { name: "Tidy up" }));
    const row = (await screen.findByText("frames-run3.csv")).closest("li");
    const inRow = within(row ?? document.body);
    await user.click(inRow.getByRole("button", { name: "Attach to a card…" }));
    await user.type(inRow.getByRole("textbox"), "ASL-99");
    await user.click(inRow.getByRole("button", { name: "Attach" }));
    expect(await inRow.findByRole("alert")).toHaveTextContent("no card ASL-99");
  });

  it("only lists them to a connection that can't change cards", async () => {
    const user = userEvent.setup();
    localStorage.setItem("flotilla.artifacts.kept.p1", '["notes-0917.md", 3]');
    show(grouped(), false);
    await user.click(await screen.findByRole("tab", { name: "Tidy up" }));
    expect(await screen.findByText("frames-run3.csv")).toBeInTheDocument();
    expect(screen.queryByText("notes-0917.md")).toBeNull();
    expect(screen.queryByRole("button", { name: "Keep" })).toBeNull();
  });

  it("says when every shared file is on a card or kept", async () => {
    const user = userEvent.setup();
    show(grouped(GROUPED_ARTIFACTS.filter((a) => (a.used_by ?? []).length > 0)));
    await user.click(await screen.findByRole("tab", { name: "Tidy up" }));
    expect(await screen.findByText("Every shared file is on a card or kept.")).toBeInTheDocument();
  });
});

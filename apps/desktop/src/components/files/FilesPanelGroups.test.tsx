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

/** The approve grant, with the daemon's keep and trash (step 3's shapes). */
function owner(fake: FakeDaemon, bin: "os" | "folder" = "os"): FakeDaemon {
  fake.grants = ["read", "control", "approve"];
  return fake
    .onRequest("artifact_keep", (body) => {
      const rel = body.type === "artifact_keep" ? body.path : "";
      const listed = GROUPED_ARTIFACTS.find((a) => a.rel === rel) ?? GROUPED_ARTIFACTS[0];
      return {
        type: "artifact_kept",
        req_id: "2",
        project_id: "p1",
        artifact: { ...(listed ?? { path: rel, rel, name: rel, size: 0, mime: "" }), kept: true },
      };
    })
    .onRequest("artifact_trash", (body) => ({
      type: "artifact_trashed",
      req_id: "3",
      project_id: "p1",
      path: body.type === "artifact_trash" ? body.path : "",
      bin,
    }));
}

async function openTidy(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(await screen.findByRole("tab", { name: "Tidy up" }));
  const row = (await screen.findByText(name)).closest("li");
  return within(row ?? document.body);
}

describe("tidy up", () => {
  it("lists the shared files no card uses and nobody kept, and keeps one", async () => {
    const user = userEvent.setup();
    const fake = owner(grouped());
    show(fake);
    const row = await openTidy(user, "notes-0917.md");
    expect(screen.queryByText("Saints dataset")).toBeNull();
    expect(screen.queryByText("style-guide.md")).toBeNull();
    expect(screen.getByText("frames-run3.csv")).toBeInTheDocument();
    await user.click(row.getByRole("button", { name: "Keep" }));
    expect(await screen.findByRole("status")).toHaveTextContent("Kept notes-0917.md.");
    expect(screen.queryByText("notes-0917.md")).toBeNull();
    expect(fake.requests.map((r) => r.body)).toContainEqual({
      type: "artifact_keep",
      project_id: "p1",
      path: "notes-0917.md",
      keep: true,
    });
    await waitFor(() => expect(lists(fake).length).toBeGreaterThan(1));
  });

  it("removes a file to the trash only once confirmed", async () => {
    const user = userEvent.setup();
    const fake = owner(grouped());
    show(fake);
    const row = await openTidy(user, "frames-run3.csv");
    await user.click(row.getByRole("button", { name: "Remove…" }));
    const dialog = screen.getByRole("dialog", { name: "Remove frames-run3.csv?" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(fake.requests.some((r) => r.body.type === "artifact_trash")).toBe(false);
    await user.click(row.getByRole("button", { name: "Remove…" }));
    await user.click(screen.getByRole("button", { name: "Move to trash" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Moved frames-run3.csv to the trash.",
    );
    expect(fake.requests.map((r) => r.body)).toContainEqual({
      type: "artifact_trash",
      project_id: "p1",
      path: "runs/frames-run3.csv",
    });
  });

  it("says when a file went to Flotilla's trash folder", async () => {
    const user = userEvent.setup();
    show(owner(grouped(), "folder"));
    const row = await openTidy(user, "notes-0917.md");
    await user.click(row.getByRole("button", { name: "Remove…" }));
    await user.click(screen.getByRole("button", { name: "Move to trash" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Moved notes-0917.md to Flotilla's trash folder.",
    );
  });

  it("says why the daemon refused to remove a file", async () => {
    const user = userEvent.setup();
    const fake = owner(grouped()).onRequest("artifact_trash", () => {
      throw new DaemonError("conflict", "a live card uses it");
    });
    show(fake);
    const row = await openTidy(user, "frames-run3.csv");
    await user.click(row.getByRole("button", { name: "Remove…" }));
    await user.click(screen.getByRole("button", { name: "Move to trash" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "frames-run3.csv: a live card uses it",
    );
    expect(screen.getByText("frames-run3.csv")).toBeInTheDocument();
  });

  it("attaches a file to a card, then reads the list again", async () => {
    const user = userEvent.setup();
    const fake = grouped().onBoard("itemAttach", () => ({
      case: "item",
      value: create(ItemDetailSchema, {}),
    }));
    show(fake);
    const row = await openTidy(user, "frames-run3.csv");
    expect(row.queryByRole("button", { name: "Keep" })).toBeNull();
    await user.click(row.getByRole("button", { name: "Attach to a card…" }));
    await user.click(row.getByRole("button", { name: "Cancel" }));
    await user.click(row.getByRole("button", { name: "Attach to a card…" }));
    await user.type(row.getByRole("textbox", { name: /Card to attach/ }), " ASL-9 ");
    await user.click(row.getByRole("button", { name: "Attach" }));
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
    const row = await openTidy(user, "frames-run3.csv");
    await user.click(row.getByRole("button", { name: "Attach to a card…" }));
    await user.type(row.getByRole("textbox"), "ASL-99");
    await user.click(row.getByRole("button", { name: "Attach" }));
    expect(await row.findByRole("alert")).toHaveTextContent("no card ASL-99");
  });

  it("only lists them to a connection that can't change cards", async () => {
    const user = userEvent.setup();
    show(owner(grouped()), false);
    const row = await openTidy(user, "frames-run3.csv");
    expect(row.queryByRole("button")).toHaveAccessibleName(/frames-run3/);
  });

  it("says when every shared file is on a card or kept", async () => {
    const user = userEvent.setup();
    show(grouped(GROUPED_ARTIFACTS.filter((a) => (a.used_by ?? []).length > 0 || a.kept)));
    await user.click(await screen.findByRole("tab", { name: "Tidy up" }));
    expect(await screen.findByText("Every shared file is on a card or kept.")).toBeInTheDocument();
  });
});

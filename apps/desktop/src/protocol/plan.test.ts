import { create } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import { FakeDaemon } from "../test/fakeDaemon";
import { PlanDocContentSchema } from "./gen/flotilla/board/v1/plan_pb";
import { isTextMime, planFileOf, readPlanDoc } from "./plan";

function chunk(text: string, total: number, mime = "text/markdown", diff?: string) {
  return create(PlanDocContentSchema, {
    path: "docs/prd.md",
    sha: "a8d3e55",
    data: new TextEncoder().encode(text),
    total: BigInt(total),
    mime,
    diff,
  });
}

describe("reading a plan document", () => {
  it("reads every chunk at the commit of the first, the diff with the first", async () => {
    const parts = ["# Combat ", "readability\n"];
    const fake = new FakeDaemon().onBoard("planDocRead", (call) => {
      const offset = call.case === "planDocRead" ? Number(call.value.offset ?? 0n) : 0;
      const part = offset === 0 ? parts[0] : parts[1];
      return { case: "planDoc", value: chunk(part ?? "", 21, "text/markdown", "@@ -1 +1 @@") };
    });
    const file = await readPlanDoc(fake, "p1", "docs/prd.md", { diffFrom: "77ad310" });
    expect(file).toEqual({
      mime: "text/markdown",
      sha: "a8d3e55",
      text: "# Combat readability\n",
      diff: "@@ -1 +1 @@",
    });
    const calls = fake.boardCalls.map((c) => (c.case === "planDocRead" ? c.value : null));
    expect(calls).toMatchObject([
      { offset: 0n, diffFrom: "77ad310" },
      { offset: 9n, sha: "a8d3e55", diffFrom: undefined },
    ]);
  });

  it("stops at an empty chunk, and keeps images as base64", () => {
    const image = chunk("PNG", 3, "image/png");
    expect(planFileOf(image, [image.data])).toEqual({
      mime: "image/png",
      sha: "a8d3e55",
      base64: btoa("PNG"),
    });
    expect(isTextMime("text/html")).toBe(true);
    expect(isTextMime("image/svg+xml")).toBe(true);
    expect(isTextMime("application/json")).toBe(true);
    expect(isTextMime("application/pdf")).toBe(false);
  });

  it("ends early when the service has no more to give", async () => {
    const fake = new FakeDaemon().onBoard("planDocRead", (call) => {
      const first = call.case === "planDocRead" && call.value.offset === 0n;
      return { case: "planDoc", value: chunk(first ? "abc" : "", 10) };
    });
    expect((await readPlanDoc(fake, "p1", "docs/prd.md")).text).toBe("abc");
  });
});

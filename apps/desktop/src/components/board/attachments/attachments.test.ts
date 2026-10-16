import { create } from "@bufbuild/protobuf";
import { describe, expect, it } from "vitest";
import { ItemAttachmentSchema } from "../../../protocol/gen/flotilla/board/v1/board_pb";
import {
  AttachmentChunkSchema,
  UploadProgressSchema,
  ItemDetailSchema,
} from "../../../protocol/gen/flotilla/board/v1/requests_pb";
import { draftDetail, SCREENSHOT_PNG } from "../../../test/boardsFixtures";
import { FakeDaemon } from "../../../test/fakeDaemon";
import {
  attachedBytes,
  attachmentId,
  CAPS,
  docSource,
  inlineImage,
  isImageName,
  kindLabel,
  newUploadId,
  referencedIds,
  refusalFor,
  sizeWords,
} from "./attachments";
import { dataUrl, readAttachment, uploadAttachment } from "./transfer";

const attachments = draftDetail().attachments;

const storedFile = (id: string, path: string, bytes: bigint, removed = false) =>
  create(ItemAttachmentSchema, { id, kind: "file", name: "a.pdf", path, bytes, removed });

describe("what an item may carry", () => {
  it("refuses a type, a size or an item total at once, with the reason", () => {
    expect(refusalFor("shot.png", 1000, 0)).toBeNull();
    expect(refusalFor("IMG.HEIC", 1000, 0)).toMatch(/JPEG/);
    expect(refusalFor("tool.exe", 1000, 0)).toMatch(/\.exe isn't taken/);
    expect(refusalFor("Makefile", 10, 0)).toMatch(/without a type/);
    expect(refusalFor("big.png", CAPS.image + 1, 0)).toMatch(/cap for an image/);
    expect(refusalFor("big.pdf", CAPS.file + 1, 0)).toMatch(/cap for a file/);
    expect(refusalFor("a.pdf", 1000, CAPS.item)).toMatch(/item would hold/);
    expect(isImageName("x.WEBP")).toBe(true);
  });

  it("words sizes and kinds", () => {
    expect(sizeWords(1)).toBe("1 byte");
    expect(sizeWords(2048)).toBe("2 KB");
    expect(sizeWords(1_310_720)).toBe("1.3 MB");
    expect(attachments.map((a) => kindLabel(a).word)).toEqual([
      "image",
      "pdf",
      "document",
      "image",
    ]);
    expect(kindLabel(create(ItemAttachmentSchema, { kind: "doc", url: "https://x" })).word).toBe(
      "link",
    );
    expect(kindLabel(create(ItemAttachmentSchema, { kind: "file", name: "notes" })).word).toBe(
      "file",
    );
    // Docs don't count toward the cap; the fixture's detached image has no stored copy.
    expect(attachedBytes(attachments)).toBe(194 + 1_310_720);
  });

  it("counts what the item stores as the service does: detached files too, a copy once", () => {
    const stored = [
      storedFile("a", "items/RB-1/aa-a.pdf", 60n * 1024n * 1024n, true),
      storedFile("b", "items/RB-1/bb-b.pdf", 30n * 1024n * 1024n),
      storedFile("c", "items/RB-1/bb-b.pdf", 30n * 1024n * 1024n),
    ];
    expect(attachedBytes(stored)).toBe(90 * 1024 * 1024);
    // So 20 MB more is refused here, before a byte is sent, as the service would.
    expect(refusalFor("c.pdf", 20 * 1024 * 1024, attachedBytes(stored))).toMatch(/item would hold/);
  });

  it("refuses an empty file at once, which the service doesn't take", () => {
    expect(refusalFor("empty.txt", 0, 0)).toMatch(/empty/);
  });

  it("reads attachment references in a description", () => {
    expect(referencedIds(draftDetail().item?.description ?? "")).toEqual(["att-shot", "att-gone"]);
    expect(attachmentId("attachment:att-1")).toBe("att-1");
    expect(attachmentId("https://x/y.png")).toBeNull();
    const shot = attachments[0];
    expect(shot && inlineImage(shot)).toBe("![The error](attachment:att-shot)");
    expect(newUploadId(36, 0.5)).toMatch(/^up-10-[a-z0-9]+$/);
    expect(docSource("https://example.com/a")).toEqual({ url: "https://example.com/a" });
    expect(docSource("/specs/a.md")).toEqual({ artifact: "specs/a.md" });
    expect(docSource("  ")).toBeNull();
  });
});

describe("moving an attachment's bytes", () => {
  it("uploads in chunks of at most 512 KiB, reporting progress, then attaches", async () => {
    const offsets: bigint[] = [];
    const fake = new FakeDaemon()
      .onBoard("attachmentUpload", (call) => {
        if (call.case !== "attachmentUpload") {
          throw new Error("unexpected");
        }
        offsets.push(call.value.offset ?? 0n);
        const received = (call.value.offset ?? 0n) + BigInt(call.value.data?.length ?? 0);
        return {
          case: "upload",
          value: create(UploadProgressSchema, { received }),
        };
      })
      .onBoard("itemAttach", () => ({ case: "item", value: create(ItemDetailSchema, {}) }));
    const bytes = new Uint8Array(1_200_000);
    const seen: number[] = [];
    const file = {
      name: "log.txt",
      size: bytes.length,
      slice: (a: number, b: number) => new Blob([bytes.slice(a, b)]),
    };
    await uploadAttachment(fake, "RB-20", "u", file, (n) => seen.push(n), { caption: "The log" });
    expect(offsets).toEqual([0n, 524_288n, 1_048_576n]);
    expect(seen.at(-1)).toBe(1_200_000);
    const attach = fake.boardCalls.at(-1);
    expect(attach?.case === "itemAttach" && attach.value).toMatchObject({
      uploadId: "u",
      name: "log.txt",
      caption: "The log",
    });
  });

  it("reads an attachment back a chunk at a time", async () => {
    const half = SCREENSHOT_PNG.length / 2;
    const fake = new FakeDaemon().onBoard("attachmentRead", (call) => {
      const offset = call.case === "attachmentRead" ? Number(call.value.offset ?? 0n) : 0;
      return {
        case: "attachment",
        value: create(AttachmentChunkSchema, {
          data: SCREENSHOT_PNG.slice(offset, offset + half),
          total: BigInt(SCREENSHOT_PNG.length),
          mime: "image/png",
        }),
      };
    });
    const { bytes, mime } = await readAttachment(fake, "att-shot");
    expect(bytes).toEqual(SCREENSHOT_PNG);
    expect(dataUrl(bytes, mime)).toMatch(/^data:image\/png;base64,iVBOR/);
    expect(dataUrl(new Uint8Array(), "")).toBe("data:application/octet-stream;base64,");
  });
});

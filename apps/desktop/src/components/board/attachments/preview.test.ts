import { describe, expect, it } from "vitest";
import { folderOf, inlinePage, kindOf, PAGE_POLICY, resolve, type ReadFile } from "./preview";

describe("a card's documents", () => {
  it("reads Markdown, HTML, PDFs and text by type or name", () => {
    expect(kindOf("text/markdown", "spec")).toBe("markdown");
    expect(kindOf("", "docs/SPEC.md")).toBe("markdown");
    expect(kindOf("text/html", "report")).toBe("html");
    expect(kindOf("", "report.HTM")).toBe("html");
    expect(kindOf("application/pdf", "a")).toBe("pdf");
    expect(kindOf("text/plain", "build.log")).toBe("text");
    expect(kindOf("image/png", "shot.png")).toBe("image");
    expect(kindOf("application/zip", "a.zip")).toBeNull();
  });

  it("finds a page's files beside it, never above the root", () => {
    expect(folderOf("reports/run 1/index.html")).toBe("reports/run 1");
    expect(folderOf("index.html")).toBe("");
    expect(resolve("reports/run", "style.css")).toBe("reports/run/style.css");
    expect(resolve("reports/run", "../shared/a%20b.png?v=2")).toBe("reports/shared/a b.png");
    expect(resolve("reports", "/top.css")).toBe("top.css");
    expect(resolve("", "../escape.css")).toBeNull();
    expect(resolve("/home/a/bot/workspace", "out/s.css")).toBe("/home/a/bot/workspace/out/s.css");
    expect(resolve("/home/a/bot", "/etc/passwd")).toBeNull();
    for (const outside of [
      "https://cdn.example/x.js",
      "//cdn.example/x.js",
      "data:image/png;base64,AA",
      "#top",
      "",
    ]) {
      expect(resolve("r", outside)).toBeNull();
    }
  });

  it("puts a page's styles, scripts and images inside it, and keeps it off the network", async () => {
    const files: Record<string, ReadFile> = {
      "r/style.css": { mime: "text/css", text: "h1 { color: teal }" },
      "r/chart.js": { mime: "text/javascript", text: "window.drawn = true;" },
      "r/plot.png": { mime: "image/png", base64: "iVBORw0K" },
    };
    const read = async (path: string): Promise<ReadFile> => {
      const file = files[path];
      if (file === undefined) {
        throw new Error(`no ${path}`);
      }
      return file;
    };
    const page = await inlinePage(
      `<html><head><link rel="stylesheet" href="style.css"><link rel="stylesheet" href="missing.css"></head>
       <body><h1>Run</h1><img src="plot.png"><img src="https://tracker.example/p.gif">
       <script src="chart.js"></script></body></html>`,
      "r/index.html",
      read,
    );
    const doc = new DOMParser().parseFromString(page, "text/html");
    expect(doc.querySelector("style")?.textContent).toBe("h1 { color: teal }");
    expect(doc.querySelectorAll("link").length).toBe(0);
    expect(doc.querySelector("script")?.textContent).toBe("window.drawn = true;");
    expect(doc.querySelector("script")?.hasAttribute("src")).toBe(false);
    const images = Array.from(doc.querySelectorAll("img")).map((i) => i.getAttribute("src"));
    expect(images[0]).toBe("data:image/png;base64,iVBORw0K");
    // Left as it is; the policy keeps it from loading.
    expect(images[1]).toBe("https://tracker.example/p.gif");
    const policy = doc.head.firstElementChild;
    expect(policy?.getAttribute("http-equiv")).toBe("Content-Security-Policy");
    expect(policy?.getAttribute("content")).toBe(PAGE_POLICY);
  });

  it("shows a page with no way to read its files as it is, with the policy", async () => {
    const page = await inlinePage("<p>Hi</p>", "a.html", null);
    expect(page).toContain("<p>Hi</p>");
    expect(page).toContain("Content-Security-Policy");
  });
});

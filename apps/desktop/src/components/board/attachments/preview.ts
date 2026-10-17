// Reading a card's documents in the app (0.20.0): Markdown as formatted
// text, an HTML page in a sandbox, plain text and PDFs. A page's relative
// files (its CSS, images, scripts) are read through the same connection and
// put inside it, so it renders without any server and reaches nothing else.

/** What the viewer shows for one document. */
export type Preview =
  | { readonly kind: "markdown"; readonly text: string }
  | { readonly kind: "html"; readonly page: string }
  | { readonly kind: "text"; readonly text: string }
  | { readonly kind: "pdf"; readonly bytes: Uint8Array }
  | { readonly kind: "image"; readonly url: string }
  | { readonly kind: "none"; readonly why: string };

/** A file as `read_file` answers: text, or base64 for images and PDFs. */
export interface ReadFile {
  readonly mime: string;
  readonly text?: string;
  readonly base64?: string;
}

/** Reads a file by its path in the same folder tree as the page. */
export type Reader = (path: string) => Promise<ReadFile>;

/**
 * Nothing in the page reaches the network: its own inline styles and scripts
 * run, and what it shows comes from inside it (data: URLs). The sandbox
 * around it keeps it off the app (no same origin).
 */
export const PAGE_POLICY =
  "default-src 'none'; img-src data: blob:; media-src data: blob:; font-src data:; " +
  "style-src 'unsafe-inline'; script-src 'unsafe-inline'";

/** The kind a MIME type or a name reads as. */
export function kindOf(
  mime: string,
  name: string,
): "markdown" | "html" | "text" | "pdf" | "image" | null {
  const lower = name.toLowerCase();
  if (mime === "text/markdown" || lower.endsWith(".md") || lower.endsWith(".markdown")) {
    return "markdown";
  }
  if (mime === "text/html" || lower.endsWith(".html") || lower.endsWith(".htm")) {
    return "html";
  }
  if (mime === "application/pdf" || lower.endsWith(".pdf")) {
    return "pdf";
  }
  if (mime.startsWith("image/")) {
    return "image";
  }
  if (mime.startsWith("text/") || /\.(txt|log|json|csv|ya?ml|toml)$/u.test(lower)) {
    return "text";
  }
  return null;
}

/** The folder a path is in: `reports/run 1/index.html` → `reports/run 1`. */
export function folderOf(path: string): string {
  const at = path.lastIndexOf("/");
  return at < 0 ? "" : path.slice(0, at);
}

/**
 * A page's reference as a path beside it, or null when it isn't one: a
 * scheme, `//host`, a fragment or a `data:` URL. `..` steps up, never above
 * the tree's root.
 */
export function resolve(folder: string, ref: string): string | null {
  const href = ref.trim().split(/[?#]/u)[0] ?? "";
  // A folder on disk (a bot's file) keeps its root; there `/x` isn't beside the page.
  const absolute = folder.startsWith("/");
  if (!isBeside(href) || (absolute && href.startsWith("/"))) {
    return null;
  }
  const start = href.startsWith("/") ? [] : folder.split("/").filter((p) => p !== "");
  const parts = walk(start, href.split("/"));
  if (parts === null || parts.length === 0) {
    return null;
  }
  return `${absolute ? "/" : ""}${parts.join("/")}`;
}

/** Not a scheme, `//host`, a fragment or empty: a path. */
function isBeside(href: string): boolean {
  return href !== "" && !/^[a-z][a-z0-9+.-]*:/iu.test(href) && !href.startsWith("//");
}

/** `parts` after stepping through `steps`; null when `..` leaves the root. */
function walk(parts: readonly string[], steps: readonly string[]): string[] | null {
  const out = [...parts];
  for (const step of steps) {
    if (step === "..") {
      if (out.pop() === undefined) {
        return null;
      }
    } else if (step !== "" && step !== ".") {
      out.push(decodeURIComponentSafe(step));
    }
  }
  return out;
}

function decodeURIComponentSafe(part: string): string {
  try {
    return decodeURIComponent(part);
  } catch {
    return part;
  }
}

/** A file as a `data:` URL. */
function asDataUrl(file: ReadFile): string {
  if (file.base64 !== undefined) {
    return `data:${file.mime || "application/octet-stream"};base64,${file.base64}`;
  }
  return `data:${file.mime || "text/plain"};charset=utf-8,${encodeURIComponent(file.text ?? "")}`;
}

/** Puts the page's policy first in its head. */
function withPolicy(doc: Document): void {
  const meta = doc.createElement("meta");
  meta.setAttribute("http-equiv", "Content-Security-Policy");
  meta.setAttribute("content", PAGE_POLICY);
  doc.head.insertBefore(meta, doc.head.firstChild);
}

/**
 * The page with its relative stylesheets and scripts written into it and
 * its images as `data:` URLs, read with `read` from beside `path`. A file
 * that can't be read is left out; the page shows without it.
 */
export async function inlinePage(html: string, path: string, read: Reader | null): Promise<string> {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const folder = folderOf(path);
  const cache = new Map<string, Promise<ReadFile | null>>();
  const fetchFile = (ref: string | null): Promise<ReadFile | null> => {
    if (read === null || ref === null) {
      return Promise.resolve(null);
    }
    const target = resolve(folder, ref);
    if (target === null) {
      return Promise.resolve(null);
    }
    let pending = cache.get(target);
    if (pending === undefined) {
      pending = read(target).catch(() => null);
      cache.set(target, pending);
    }
    return pending;
  };
  const inlineStyle = async (link: Element): Promise<void> => {
    const file = await fetchFile(link.getAttribute("href"));
    if (file?.text === undefined) {
      link.remove();
      return;
    }
    const style = doc.createElement("style");
    style.textContent = file.text;
    link.replaceWith(style);
  };
  const inlineScript = async (script: Element): Promise<void> => {
    const file = await fetchFile(script.getAttribute("src"));
    script.removeAttribute("src");
    script.textContent = file?.text ?? "";
  };
  const inlineMedia = async (media: Element): Promise<void> => {
    const file = await fetchFile(media.getAttribute("src"));
    if (file !== null) {
      media.setAttribute("src", asDataUrl(file));
    }
  };
  const stylesheets = Array.from(doc.querySelectorAll("link[href]")).filter((link) =>
    (link.getAttribute("rel") ?? "").toLowerCase().split(/\s+/u).includes("stylesheet"),
  );
  const work: Promise<void>[] = [
    ...stylesheets.map(inlineStyle),
    ...Array.from(doc.querySelectorAll("script[src]")).map(inlineScript),
    ...Array.from(doc.querySelectorAll("img[src], source[src], video[src], audio[src]")).map(
      inlineMedia,
    ),
  ];
  await Promise.all(work);
  withPolicy(doc);
  return `<!doctype html>\n${doc.documentElement.outerHTML}`;
}

/** UTF-8 text of an attachment's bytes. */
export function textOf(bytes: Uint8Array): string {
  return new TextDecoder("utf-8", { fatal: false }).decode(bytes);
}

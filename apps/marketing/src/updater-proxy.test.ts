import { afterEach, describe, expect, it, vi } from "vitest";

import { proxyUpdateManifest } from "./updater-proxy";

const upstream = "https://github.com/example/app/releases/latest/download/latest.json";
const current = "https://updates.example/desktop/flotilla/latest.json";
// Pre-rename path that installed apps keep polling; it must answer exactly like the current one.
const legacy = "https://updates.example/desktop/gravity/latest.json";
const body = JSON.stringify({
  version: "0.12.4",
  platforms: {
    "darwin-aarch64": {
      url: "https://github.com/example/app/releases/download/v0.12.4/flotilla.app.tar.gz",
      signature: "original-signature",
    },
  },
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe.each([
  ["current", current],
  ["legacy", legacy],
])("update manifest bridge on the %s path", (_label, manifestUrl) => {
  it("preserves manifest bytes and does not forward caller credentials or query parameters", async () => {
    const fetch = vi.fn<typeof globalThis.fetch>().mockResolvedValue(new Response(body));
    vi.stubGlobal("fetch", fetch);
    const response = await proxyUpdateManifest(
      new Request(`${manifestUrl}?old-client=true`, {
        headers: { authorization: "private-client-token" },
      }),
      upstream,
    );

    expect(response.status).toBe(200);
    expect(await response.text()).toBe(body);
    expect(response.headers.get("content-type")).toBe("application/json");
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(fetch).toHaveBeenCalledExactlyOnceWith(upstream, { redirect: "follow" });
  });

  it("supports HEAD without a response body", async () => {
    vi.stubGlobal("fetch", vi.fn<typeof globalThis.fetch>().mockResolvedValue(new Response(body)));
    const response = await proxyUpdateManifest(
      new Request(manifestUrl, { method: "HEAD" }),
      upstream,
    );
    expect(response.status).toBe(200);
    expect(await response.text()).toBe("");
  });

  it.each([404, 403, 500])("fails without caching an upstream HTTP %i", async (status) => {
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof globalThis.fetch>().mockResolvedValue(new Response("unavailable", { status })),
    );
    const response = await proxyUpdateManifest(new Request(manifestUrl), upstream);
    expect(response.status).toBe(502);
    expect(response.headers.get("cache-control")).toBe("no-store");
  });

  it.each([
    "<html>error</html>",
    "{}",
    '{"platforms":{"darwin-aarch64":{"url":"bad.app.tar.gz"}}}',
  ])("rejects invalid upstream manifests", async (invalid) => {
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof globalThis.fetch>().mockResolvedValue(new Response(invalid)),
    );
    expect((await proxyUpdateManifest(new Request(manifestUrl), upstream)).status).toBe(502);
  });

  it("handles an upstream network failure", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof globalThis.fetch>().mockRejectedValue(new Error("offline")),
    );
    expect((await proxyUpdateManifest(new Request(manifestUrl), upstream)).status).toBe(502);
  });

  it("does not fetch for other paths, unsupported methods or missing configuration", async () => {
    const fetch = vi.fn<typeof globalThis.fetch>();
    vi.stubGlobal("fetch", fetch);
    expect((await proxyUpdateManifest(new Request(`${manifestUrl}/other`), upstream)).status).toBe(
      404,
    );
    expect(
      (await proxyUpdateManifest(new Request(manifestUrl, { method: "POST" }), upstream)).status,
    ).toBe(405);
    expect((await proxyUpdateManifest(new Request(manifestUrl), "")).status).toBe(503);
    expect(fetch).not.toHaveBeenCalled();
  });
});

describe("update manifest paths", () => {
  it("answers the legacy path exactly like the current one", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof globalThis.fetch>().mockImplementation(async () => new Response(body)),
    );
    const responses = await Promise.all(
      [current, legacy].map((url) => proxyUpdateManifest(new Request(url), upstream)),
    );
    const [fromCurrent, fromLegacy] = await Promise.all(
      responses.map(async (response) => ({
        status: response.status,
        headers: [...response.headers],
        body: await response.text(),
      })),
    );
    expect(fromLegacy).toStrictEqual(fromCurrent);
  });

  it.each([
    "https://updates.example/desktop/latest.json",
    "https://updates.example/desktop/other/latest.json",
    "https://updates.example/desktop/Flotilla/latest.json",
    "https://updates.example/desktop/flotilla/latest.json/",
  ])("rejects unknown manifest path %s", async (url) => {
    const fetch = vi.fn<typeof globalThis.fetch>();
    vi.stubGlobal("fetch", fetch);
    expect((await proxyUpdateManifest(new Request(url), upstream)).status).toBe(404);
    expect(fetch).not.toHaveBeenCalled();
  });
});

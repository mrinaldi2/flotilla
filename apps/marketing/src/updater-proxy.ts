import { resolveMacDownloadUrl } from "./release";

// The Flotilla path, plus the pre-rename path that already installed apps keep polling.
// Both answer identically; never drop the legacy alias.
const MANIFEST_PATHS: ReadonlySet<string> = new Set([
  "/desktop/flotilla/latest.json",
  "/desktop/gravity/latest.json",
]);

function unavailable(): Response {
  return new Response("The update manifest is temporarily unavailable.", {
    status: 502,
    headers: { "cache-control": "no-store" },
  });
}

export async function proxyUpdateManifest(
  request: Request,
  manifestUrl: string | undefined,
): Promise<Response> {
  if (!MANIFEST_PATHS.has(new URL(request.url).pathname)) {
    return new Response("Not found", { status: 404 });
  }
  if (request.method !== "GET" && request.method !== "HEAD") {
    return new Response("Method not allowed", { status: 405, headers: { allow: "GET, HEAD" } });
  }
  if (!manifestUrl?.trim()) {
    return new Response("Updates are not configured.", { status: 503 });
  }

  try {
    // Never forward caller cookies, authorization, query parameters or validators.
    const upstream = await fetch(manifestUrl, { redirect: "follow" });
    if (!upstream.ok) {
      return unavailable();
    }
    const body = await upstream.text();
    const manifest: unknown = JSON.parse(body);
    if (resolveMacDownloadUrl(manifest) === null) {
      return unavailable();
    }
    // Preserve the manifest bytes, especially the signed archive's signature.
    return new Response(request.method === "HEAD" ? null : body, {
      headers: { "content-type": "application/json", "cache-control": "no-store" },
    });
  } catch {
    return unavailable();
  }
}

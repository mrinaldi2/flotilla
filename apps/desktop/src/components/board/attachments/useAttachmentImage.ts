// An attached image's bytes as a `data:` URL, read once per attachment and
// kept for the session: the description and the thumbnails share it. The
// bytes come from the board's home, never from a remote host, so showing
// one tells nobody the owner looked.

import { useEffect, useState } from "react";
import type { BoardApi } from "../../../protocol/board";
import { errText } from "../../../util";
import { dataUrl, readAttachment } from "./transfer";

export type ImageState =
  | { readonly kind: "loading" }
  | { readonly kind: "ready"; readonly url: string }
  | { readonly kind: "failed"; readonly message: string };

const cache = new Map<string, Promise<string>>();

function load(api: BoardApi, id: string): Promise<string> {
  let pending = cache.get(id);
  if (pending === undefined) {
    pending = readAttachment(api, id).then(({ bytes, mime }) => dataUrl(bytes, mime));
    // A failed read is tried again next time.
    pending.catch(() => cache.delete(id));
    cache.set(id, pending);
  }
  return pending;
}

export function useAttachmentImage(api: BoardApi, id: string | null): ImageState {
  const [state, setState] = useState<ImageState>({ kind: "loading" });
  useEffect(() => {
    if (id === null) {
      return undefined;
    }
    let live = true;
    load(api, id).then(
      (url) => live && setState({ kind: "ready", url }),
      (failure: unknown) => live && setState({ kind: "failed", message: errText(failure) }),
    );
    return () => {
      live = false;
    };
  }, [api, id]);
  return state;
}

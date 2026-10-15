// The link a card id becomes in Markdown (UX-035 §1).

/** The scheme card links are written with. */
export const SCHEME = "flotilla://item/";

/** Card links written before the rename; still read as card links. */
const LEGACY_SCHEMES: readonly string[] = ["hermes://item/", "thehermes://item/"];

/** The card id a written card link names, under any scheme, or `null`. */
export function linkedId(href: string | undefined): string | null {
  if (href === undefined) {
    return null;
  }
  const scheme = [SCHEME, ...LEGACY_SCHEMES].find((s) => href.startsWith(s));
  return scheme === undefined ? null : href.slice(scheme.length);
}

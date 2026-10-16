/**
 * localStorage under the `flotilla.` namespace. Values saved before the
 * renames live under `hermes.` and `gravity.` (newest first) in the same
 * WebView origin, which the app carries over to its new bundle id on first
 * launch, so reads fall back to them until the key is next written. Callers
 * keep their own try/catch: storage can be unavailable.
 */
const PREFIX = "flotilla.";
const LEGACY_PREFIXES: readonly string[] = ["hermes.", "gravity."];

export function readStored(name: string): string | null {
  const current = localStorage.getItem(PREFIX + name);
  if (current !== null) {
    return current;
  }
  for (const prefix of LEGACY_PREFIXES) {
    const legacy = localStorage.getItem(prefix + name);
    if (legacy !== null) {
      return legacy;
    }
  }
  return null;
}

export function writeStored(name: string, value: string): void {
  localStorage.setItem(PREFIX + name, value);
}

/** Removes every spelling, so a legacy value cannot reappear as a fallback. */
export function removeStored(name: string): void {
  localStorage.removeItem(PREFIX + name);
  for (const prefix of LEGACY_PREFIXES) {
    localStorage.removeItem(prefix + name);
  }
}

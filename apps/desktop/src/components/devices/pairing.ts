import type { DaemonConfig } from "../../protocol/entities";

/**
 * Building the link the Flotilla phone app pairs from (H-010).
 *
 * The phone reads `flotilla://pair?host=…&port=…&token=…`, optionally with
 * `name` and `kind` (`mac` or `windows`); an IPv6 host may be bracketed. It
 * reads query values with `URLComponents`, which percent-decodes but leaves
 * `+` alone, so every value is encoded with `encodeURIComponent`.
 */

/** Which network an address is on, in the order a phone should prefer. */
export type PairingNetwork = "tailscale" | "lan" | "other";

export interface PairingHost {
  readonly address: string;
  readonly network: PairingNetwork;
}

export type PairingKind = "mac" | "windows";

/** Where a phone reaches this computer, and what to call it. */
export interface PairingTarget {
  /** Reachable addresses, Tailscale first. Empty: no address a phone can use. */
  readonly hosts: readonly PairingHost[];
  readonly port: number;
  readonly name: string | null;
  readonly kind: PairingKind | null;
  /** The service listens on every interface (`0.0.0.0`/`::`), so no address is named. */
  readonly anyAddress: boolean;
  /** Where `flotillad.toml` lives on this computer, for the bind hint. */
  readonly configPath: string;
}

type AddressClass = "loopback" | "unspecified" | "link-local" | PairingNetwork;

const NETWORK_ORDER: readonly PairingNetwork[] = ["tailscale", "lan", "other"];

function ipv4Octets(address: string): readonly number[] | null {
  const parts = address.split(".");
  if (parts.length !== 4 || parts.some((part) => !/^\d{1,3}$/.test(part))) {
    return null;
  }
  const octets = parts.map(Number);
  return octets.every((octet) => octet <= 255) ? octets : null;
}

function classifyV4([a, b]: readonly number[]): AddressClass {
  if (a === 127) {
    return "loopback";
  }
  if (a === 0) {
    return "unspecified";
  }
  if (a === 169 && b === 254) {
    return "link-local";
  }
  // Tailscale hands out addresses from the CGNAT range, 100.64.0.0/10.
  if (a === 100 && b >= 64 && b <= 127) {
    return "tailscale";
  }
  if (a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168)) {
    return "lan";
  }
  return "other";
}

/** The first three hextets of an IPv6 address; a `::` stands for zeros. */
function leadingHextets(address: string): readonly number[] {
  const [head = ""] = address.split("::");
  const parts = head === "" ? [] : head.split(":");
  const hextets = parts.slice(0, 3).map((part) => Number.parseInt(part, 16));
  while (hextets.length < 3) {
    hextets.push(0);
  }
  return hextets;
}

function classifyV6(address: string): AddressClass {
  if (address === "::1") {
    return "loopback";
  }
  if (address === "::") {
    return "unspecified";
  }
  const [first = 0, second = 0, third = 0] = leadingHextets(address);
  if ((first & 0xffc0) === 0xfe80) {
    return "link-local";
  }
  // Tailscale's IPv6 range is fd7a:115c:a1e0::/48.
  if (first === 0xfd7a && second === 0x115c && third === 0xa1e0) {
    return "tailscale";
  }
  return (first & 0xfe00) === 0xfc00 ? "lan" : "other";
}

/** An address as the daemon serialised it, with an IPv4-mapped one unwrapped. */
function normalise(raw: string): string {
  const address = raw.trim().toLowerCase();
  const mapped = /^::ffff:(\d+\.\d+\.\d+\.\d+)$/.exec(address);
  return mapped?.[1] ?? address;
}

function classify(address: string): AddressClass {
  const octets = ipv4Octets(address);
  if (octets !== null) {
    return classifyV4(octets);
  }
  return address.includes(":") ? classifyV6(address) : "other";
}

/**
 * The bind addresses a phone can reach: never loopback or link-local,
 * Tailscale first, then the local network, IPv4 before IPv6 within each.
 */
export function reachableHosts(bind: readonly string[]): readonly PairingHost[] {
  const hosts: PairingHost[] = [];
  for (const raw of bind) {
    const address = normalise(raw);
    const kind = classify(address);
    if (kind === "tailscale" || kind === "lan" || kind === "other") {
      if (!hosts.some((host) => host.address === address)) {
        hosts.push({ address, network: kind });
      }
    }
  }
  const rank = (host: PairingHost): number =>
    NETWORK_ORDER.indexOf(host.network) * 2 + (host.address.includes(":") ? 1 : 0);
  // `sort` on our own array, not `toSorted`: the build targets ES2021.
  // oxlint-disable-next-line unicorn/no-array-sort
  hosts.sort((a, b) => rank(a) - rank(b));
  return hosts;
}

/** Whether the service listens on every interface rather than named ones. */
function listensOnAny(bind: readonly string[]): boolean {
  return bind.some((raw) => classify(normalise(raw)) === "unspecified");
}

export function pairingKind(os: string | undefined): PairingKind | null {
  if (os === "macos") {
    return "mac";
  }
  return os === "windows" ? "windows" : null;
}

/** What the daemon's config says about pairing a phone with it. */
export function pairingTarget(config: DaemonConfig): PairingTarget {
  const name = config.computer_name?.trim() ?? "";
  return {
    hosts: reachableHosts(config.bind),
    port: config.port,
    name: name === "" ? null : name,
    kind: pairingKind(config.os),
    anyAddress: listensOnAny(config.bind),
    configPath:
      config.os === "windows"
        ? String.raw`%USERPROFILE%\.flotilla\flotillad.toml`
        : "~/.flotilla/flotillad.toml",
  };
}

export interface PairingLinkParts {
  readonly host: string;
  readonly port: number;
  readonly token: string;
  readonly name: string | null;
  readonly kind: PairingKind | null;
}

/** The `flotilla://pair` link for one host. It holds the token: a secret. */
export function pairingLink(parts: PairingLinkParts): string {
  const host = parts.host.includes(":") ? `[${parts.host}]` : parts.host;
  const query: [string, string][] = [
    ["host", host],
    ["port", String(parts.port)],
    ["token", parts.token],
  ];
  if (parts.name !== null) {
    query.push(["name", parts.name]);
  }
  if (parts.kind !== null) {
    query.push(["kind", parts.kind]);
  }
  const encoded = query.map(([key, value]) => `${key}=${encodeURIComponent(value)}`).join("&");
  return `flotilla://pair?${encoded}`;
}

export const NETWORK_LABEL: Readonly<Record<PairingNetwork, string>> = {
  tailscale: "Tailscale",
  lan: "Local network",
  other: "Other network",
};

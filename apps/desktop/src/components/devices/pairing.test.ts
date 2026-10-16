import { describe, expect, it } from "vitest";
import type { DaemonConfig } from "../../protocol/entities";
import { pairingKind, pairingLink, pairingTarget, reachableHosts } from "./pairing";

function config(over: Partial<DaemonConfig> = {}): DaemonConfig {
  return {
    bind: ["127.0.0.1", "100.126.133.52"],
    port: 49777,
    configured_port: 49777,
    runtime: "pty",
    auto_compact_window: null,
    computer_name: "Studio Mac",
    os: "macos",
    ...over,
  };
}

/** The query as the phone's `URLComponents` reads it: percent-decoded, `+` kept. */
function phoneQuery(link: string): Readonly<Record<string, string>> {
  const [, query = ""] = link.split("?");
  return Object.fromEntries(
    query.split("&").map((pair) => {
      const [key = "", value = ""] = pair.split("=");
      return [key, decodeURIComponent(value)];
    }),
  );
}

describe("reachableHosts", () => {
  it("never offers loopback, link-local or the unspecified address", () => {
    expect(
      reachableHosts(["127.0.0.1", "::1", "0.0.0.0", "::", "169.254.3.4", "fe80::1", "127.8.0.1"]),
    ).toEqual([]);
  });

  it("prefers Tailscale, then the local network, then anything else", () => {
    expect(
      reachableHosts([
        "8.8.4.4",
        "192.168.1.20",
        "fd7a:115c:a1e0::5",
        "127.0.0.1",
        "100.126.133.52",
        "10.0.0.3",
      ]),
    ).toEqual([
      { address: "100.126.133.52", network: "tailscale" },
      { address: "fd7a:115c:a1e0::5", network: "tailscale" },
      { address: "192.168.1.20", network: "lan" },
      { address: "10.0.0.3", network: "lan" },
      { address: "8.8.4.4", network: "other" },
    ]);
  });

  it("knows the edges of the Tailscale and private ranges", () => {
    expect(reachableHosts(["100.64.0.1", "100.127.255.254"]).map((h) => h.network)).toEqual([
      "tailscale",
      "tailscale",
    ]);
    expect(reachableHosts(["100.63.0.1", "100.128.0.1"]).map((h) => h.network)).toEqual([
      "other",
      "other",
    ]);
    expect(
      reachableHosts(["172.16.0.1", "172.31.0.1", "172.32.0.1"]).map((h) => h.network),
    ).toEqual(["lan", "lan", "other"]);
    expect(reachableHosts(["fd00::1", "fc::1"]).map((h) => h.network)).toEqual(["lan", "other"]);
  });

  it("unwraps an IPv4-mapped address and drops duplicates", () => {
    expect(reachableHosts(["::ffff:100.70.1.2", "100.70.1.2", "::ffff:127.0.0.1"])).toEqual([
      { address: "100.70.1.2", network: "tailscale" },
    ]);
  });
});

describe("pairingTarget", () => {
  it("names the computer and its kind from the config", () => {
    const target = pairingTarget(config());
    expect(target).toEqual({
      hosts: [{ address: "100.126.133.52", network: "tailscale" }],
      port: 49777,
      name: "Studio Mac",
      kind: "mac",
      anyAddress: false,
      configPath: "~/.flotilla/flotillad.toml",
    });
  });

  it("has no host when the service is bound to loopback only", () => {
    const target = pairingTarget(config({ bind: ["127.0.0.1", "::1"] }));
    expect(target.hosts).toEqual([]);
    expect(target.anyAddress).toBe(false);
  });

  it("tells an every-address bind apart from a loopback one", () => {
    const target = pairingTarget(config({ bind: ["0.0.0.0"] }));
    expect(target.hosts).toEqual([]);
    expect(target.anyAddress).toBe(true);
  });

  it("copes with an older daemon that sends no name or OS", () => {
    const { computer_name: _name, os: _os, ...old } = config();
    const target = pairingTarget(old);
    expect(target.name).toBeNull();
    expect(target.kind).toBeNull();
    expect(pairingTarget(config({ computer_name: "  " })).name).toBeNull();
  });

  it("points Windows at its own config path", () => {
    const target = pairingTarget(config({ os: "windows" }));
    expect(target.kind).toBe("windows");
    expect(target.configPath).toBe(String.raw`%USERPROFILE%\.flotilla\flotillad.toml`);
  });
});

describe("pairingKind", () => {
  it("is mac or windows, and nothing the phone doesn't know", () => {
    expect(pairingKind("macos")).toBe("mac");
    expect(pairingKind("windows")).toBe("windows");
    expect(pairingKind("linux")).toBeNull();
    expect(pairingKind(undefined)).toBeNull();
  });
});

describe("pairingLink", () => {
  it("writes the phone's flotilla://pair format", () => {
    expect(
      pairingLink({
        host: "100.126.133.52",
        port: 49777,
        token: "abc123",
        name: "Studio",
        kind: "mac",
      }),
    ).toBe("flotilla://pair?host=100.126.133.52&port=49777&token=abc123&name=Studio&kind=mac");
  });

  it("encodes the token and name so the phone reads them back exactly", () => {
    const link = pairingLink({
      host: "pc",
      port: 1,
      token: "a+b/c=d&e?f#g h",
      name: "Manuel’s Mac & co",
      kind: "windows",
    });
    expect(link).not.toMatch(/[ +#]/);
    expect(link).toContain("token=a%2Bb%2Fc%3Dd%26e%3Ff%23g%20h");
    expect(phoneQuery(link)).toEqual({
      host: "pc",
      port: "1",
      token: "a+b/c=d&e?f#g h",
      name: "Manuel’s Mac & co",
      kind: "windows",
    });
  });

  it("brackets an IPv6 host and leaves out what is unknown", () => {
    expect(
      pairingLink({ host: "fd7a:115c:a1e0::5", port: 49777, token: "x", name: null, kind: null }),
    ).toBe("flotilla://pair?host=%5Bfd7a%3A115c%3Aa1e0%3A%3A5%5D&port=49777&token=x");
  });
});

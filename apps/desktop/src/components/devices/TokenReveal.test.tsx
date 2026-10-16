import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { toastSpy } from "../../test/spies";
import QrCode from "../QrCode";
import type { PairingTarget } from "./pairing";
import TokenReveal from "./TokenReveal";
import type { PairingState } from "./useDevices";

const TOKEN = "tok+en/=";
const TARGET: PairingTarget = {
  hosts: [
    { address: "100.126.133.52", network: "tailscale" },
    { address: "192.168.1.20", network: "lan" },
  ],
  port: 49777,
  name: "Studio Mac",
  kind: "mac",
  anyAddress: false,
  configPath: "~/.flotilla/flotillad.toml",
};
const TAILSCALE_LINK =
  "flotilla://pair?host=100.126.133.52&port=49777&token=tok%2Ben%2F%3D&name=Studio%20Mac&kind=mac";
const LAN_LINK =
  "flotilla://pair?host=192.168.1.20&port=49777&token=tok%2Ben%2F%3D&name=Studio%20Mac&kind=mac";

function renderReveal(pairing: PairingState) {
  const onToast = toastSpy();
  const onDismiss = vi.fn<() => void>();
  const writeText = vi.fn<(text: string) => Promise<void>>(() => Promise.resolve());
  vi.stubGlobal("navigator", { clipboard: { writeText } });
  render(
    <TokenReveal
      issued={{ deviceName: "phone", token: TOKEN, pairing }}
      onDismiss={onDismiss}
      onToast={onToast}
    />,
  );
  return { onToast, onDismiss, writeText };
}

/** The QR path drawn for `text`, to check what a code encodes. */
function qrPath(text: string): string | null | undefined {
  const { container, unmount } = render(<QrCode text={text} label="expected" border={4} />);
  const path = container.querySelector("path")?.getAttribute("d");
  unmount();
  return path;
}

function shownQrPath(): string | null | undefined {
  return screen
    .getByRole("img", { name: /^Pairing code for/ })
    .querySelector("path")
    ?.getAttribute("d");
}

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("TokenReveal pairing code", () => {
  it("draws the Tailscale link as a QR code by default", () => {
    renderReveal({ status: "ready", target: TARGET });

    const qr = screen.getByRole("img", {
      name: "Pairing code for Studio Mac at 100.126.133.52",
    });
    // The label describes the code without carrying the secret.
    expect(qr.getAttribute("aria-label")).not.toContain(TOKEN);
    expect(shownQrPath()).toBe(qrPath(TAILSCALE_LINK));
    expect(screen.getByText(/Add computer → Scan/)).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: /100\.126\.133\.52/ })).toBeChecked();
  });

  it("copies the pairing link and the token without repeating either", async () => {
    const user = userEvent.setup();
    const { onToast, writeText } = renderReveal({ status: "ready", target: TARGET });

    await user.click(screen.getByRole("button", { name: "Copy pairing link" }));
    expect(writeText).toHaveBeenLastCalledWith(TAILSCALE_LINK);
    expect(onToast).toHaveBeenLastCalledWith("info", "Pairing link copied", expect.any(String));

    await user.click(screen.getByRole("button", { name: "Copy token" }));
    expect(writeText).toHaveBeenLastCalledWith(TOKEN);
    expect(onToast).toHaveBeenLastCalledWith("info", "Token copied", expect.any(String));

    for (const [, title, body] of onToast.mock.calls) {
      expect(`${title} ${body}`).not.toContain(TOKEN);
      expect(`${title} ${body}`).not.toContain("flotilla://");
    }
  });

  it("reaches the copy button from the keyboard", async () => {
    const user = userEvent.setup();
    const { writeText } = renderReveal({
      status: "ready",
      target: { ...TARGET, hosts: TARGET.hosts.slice(0, 1) },
    });

    // Token field, Copy token, Dismiss, then the pairing link's button.
    await user.tab();
    await user.tab();
    await user.tab();
    await user.tab();
    expect(screen.getByRole("button", { name: "Copy pairing link" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(writeText).toHaveBeenCalledWith(TAILSCALE_LINK);
  });

  it("lets the owner pick another address", async () => {
    const user = userEvent.setup();
    const { writeText } = renderReveal({ status: "ready", target: TARGET });

    await user.click(screen.getByRole("radio", { name: /192\.168\.1\.20/ }));
    expect(
      screen.getByRole("img", { name: "Pairing code for Studio Mac at 192.168.1.20" }),
    ).toBeInTheDocument();
    expect(shownQrPath()).toBe(qrPath(LAN_LINK));

    await user.click(screen.getByRole("button", { name: "Copy pairing link" }));
    expect(writeText).toHaveBeenLastCalledWith(LAN_LINK);
  });

  it("shows a single address without a picker", () => {
    renderReveal({ status: "ready", target: { ...TARGET, hosts: TARGET.hosts.slice(1) } });
    expect(screen.queryByRole("radio")).not.toBeInTheDocument();
    expect(screen.getByText("192.168.1.20")).toBeInTheDocument();
  });

  it("explains how to bind to Tailscale instead of a useless code", () => {
    renderReveal({ status: "ready", target: { ...TARGET, hosts: [] } });

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Copy pairing link" })).not.toBeInTheDocument();
    expect(screen.getByText(/your phone can’t reach this computer yet/)).toBeInTheDocument();
    expect(screen.getByText(/listens only on this computer/)).toBeInTheDocument();
    expect(screen.getByText("~/.flotilla/flotillad.toml")).toBeInTheDocument();
    // The token itself still works by hand.
    expect(screen.getByRole("button", { name: "Copy token" })).toBeInTheDocument();
  });

  it("says why when the service listens on every address", () => {
    renderReveal({ status: "ready", target: { ...TARGET, hosts: [], anyAddress: true } });
    expect(screen.getByText(/listens on every address/)).toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("waits for the addresses, and says when they can't be read", () => {
    const { unmount } = render(
      <TokenReveal
        issued={{ deviceName: "phone", token: TOKEN, pairing: { status: "loading" } }}
        onDismiss={(): void => {}}
        onToast={(): void => {}}
      />,
    );
    expect(screen.getByText("Reading this computer’s addresses…")).toBeInTheDocument();
    unmount();

    renderReveal({ status: "failed", error: "offline" });
    expect(screen.getByText(/Couldn’t read this computer’s addresses \(offline\)/)).toBeVisible();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("never writes the link or token to the console", async () => {
    const user = userEvent.setup();
    const spies = (["log", "info", "warn", "error", "debug"] as const).map((method) =>
      vi.spyOn(console, method),
    );
    renderReveal({ status: "ready", target: TARGET });
    await user.click(screen.getByRole("button", { name: "Copy pairing link" }));
    await user.click(screen.getByRole("radio", { name: /192\.168\.1\.20/ }));
    for (const spy of spies) {
      expect(spy).not.toHaveBeenCalled();
    }
  });
});

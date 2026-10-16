import type { Story } from "@ladle/react";
import type { ReactElement } from "react";
import type { PairingTarget } from "./pairing";
import TokenReveal from "./TokenReveal";
import type { PairingState } from "./useDevices";

// A fixed, synthetic token keeps the QR code identical between runs.
const TOKEN = "grav_5f2c9e4ab8d14c6f9a3e7d1b0c8f2a6e";

const TARGET: PairingTarget = {
  hosts: [{ address: "100.126.133.52", network: "tailscale" }],
  port: 49777,
  name: "Studio Mac",
  kind: "mac",
  anyAddress: false,
  configPath: "~/.flotilla/flotillad.toml",
};

function Reveal(props: { readonly pairing: PairingState; readonly light?: boolean }): ReactElement {
  return (
    <div
      className={props.light ? "theme-light" : undefined}
      style={{ padding: 16, minHeight: "100vh" }}
    >
      <div style={{ maxWidth: 560 }}>
        <TokenReveal
          issued={{ deviceName: "phone", token: TOKEN, pairing: props.pairing }}
          onDismiss={(): void => {}}
          onToast={(): void => {}}
        />
      </div>
    </div>
  );
}

export const Default: Story = () => <Reveal pairing={{ status: "ready", target: TARGET }} />;

export const Light: Story = () => <Reveal light pairing={{ status: "ready", target: TARGET }} />;

export const SeveralAddresses: Story = () => (
  <Reveal
    pairing={{
      status: "ready",
      target: {
        ...TARGET,
        hosts: [
          { address: "100.126.133.52", network: "tailscale" },
          { address: "192.168.1.20", network: "lan" },
        ],
      },
    }}
  />
);

export const LoopbackOnly: Story = () => (
  <Reveal pairing={{ status: "ready", target: { ...TARGET, hosts: [] } }} />
);

export const LoopbackOnlyLight: Story = () => (
  <Reveal light pairing={{ status: "ready", target: { ...TARGET, hosts: [] } }} />
);

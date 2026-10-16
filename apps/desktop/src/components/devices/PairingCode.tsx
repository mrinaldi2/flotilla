import { useState } from "react";
import type { ReactElement } from "react";
import QrCode from "../QrCode";
import { NETWORK_LABEL, pairingLink } from "./pairing";
import type { PairingTarget } from "./pairing";
import type { PairingState } from "./useDevices";

interface PairingCodeProps {
  readonly token: string;
  readonly pairing: PairingState;
  /** Copies the pairing link; the caller says so without repeating it. */
  readonly onCopyLink: (link: string) => void;
}

/** About 4 px a module for a typical link: easy for a phone camera. */
const QR_SIZE = 200;
/** The quiet zone a QR code needs, in modules (ISO/IEC 18004). */
const QUIET_ZONE = 4;

const SCAN_HINT =
  "Scan with the Flotilla app on your iPhone (Add computer → Scan), or open the link on the phone.";
const BY_HAND = "Until then, enter the token on the phone by hand (Add computer → Enter manually).";

/**
 * The QR code and link a phone pairs from. Rendered only while the token is
 * shown, and never stored: it holds the token, so it goes nowhere but the
 * screen and, when asked, the clipboard.
 */
export default function PairingCode({
  token,
  pairing,
  onCopyLink,
}: PairingCodeProps): ReactElement {
  if (pairing.status === "loading") {
    return <p className="pairing-status">Reading this computer’s addresses…</p>;
  }
  if (pairing.status === "failed") {
    return (
      <div className="pairing-warning" role="status">
        <p>
          Couldn’t read this computer’s addresses ({pairing.error}), so there is no code to scan.
        </p>
        <p>Enter the token on the phone by hand (Add computer → Enter manually).</p>
      </div>
    );
  }
  const { target } = pairing;
  if (target.hosts.length === 0) {
    return <NoAddress target={target} />;
  }
  return <PairingReady target={target} token={token} onCopyLink={onCopyLink} />;
}

function PairingReady(props: {
  readonly target: PairingTarget;
  readonly token: string;
  readonly onCopyLink: (link: string) => void;
}): ReactElement {
  const { target, token, onCopyLink } = props;
  // Hosts come Tailscale first, so that is the default.
  const [chosen, setChosen] = useState(target.hosts[0]?.address ?? "");
  const host = target.hosts.find((item) => item.address === chosen) ?? target.hosts[0];
  if (host === undefined) {
    return <NoAddress target={target} />;
  }
  const link = pairingLink({
    host: host.address,
    port: target.port,
    token,
    name: target.name,
    kind: target.kind,
  });
  const computer = target.name ?? "this computer";

  return (
    <div className="pairing">
      <QrCode
        text={link}
        label={`Pairing code for ${computer} at ${host.address}`}
        size={QR_SIZE}
        border={QUIET_ZONE}
        className="pairing-qr-code"
      />
      <div className="pairing-body">
        <p className="pairing-title">Pair your iPhone</p>
        <p className="pairing-hint">{SCAN_HINT}</p>
        {target.hosts.length > 1 ? (
          <fieldset className="pairing-hosts">
            <legend>Address the phone connects to</legend>
            {target.hosts.map((item) => (
              <label key={item.address} className="member-option">
                <input
                  type="radio"
                  name="pairing-host"
                  value={item.address}
                  checked={item.address === host.address}
                  onChange={() => {
                    setChosen(item.address);
                  }}
                />
                <span className="mono">{item.address}</span>{" "}
                <span className="pairing-network">{NETWORK_LABEL[item.network]}</span>
              </label>
            ))}
          </fieldset>
        ) : (
          <p className="pairing-address">
            Address <span className="mono">{host.address}</span>{" "}
            <span className="pairing-network">{NETWORK_LABEL[host.network]}</span>
          </p>
        )}
        <div>
          <button
            type="button"
            className="btn btn-small"
            onClick={() => {
              onCopyLink(link);
            }}
          >
            Copy pairing link
          </button>
        </div>
      </div>
    </div>
  );
}

/** Bound to loopback only, or to every address: no code a phone could use. */
function NoAddress({ target }: { readonly target: PairingTarget }): ReactElement {
  const bind = (
    <>
      add this computer’s Tailscale address (<code>tailscale ip -4</code>) to <code>bind</code> in{" "}
      <code>{target.configPath}</code>, for example <code>bind = ["127.0.0.1", "100.x.y.z"]</code>,
      restart the Flotilla service, and create the device again
    </>
  );
  return (
    <div className="pairing-warning" role="status">
      <p className="pairing-title">No pairing code: your phone can’t reach this computer yet.</p>
      {target.anyAddress ? (
        <p>
          The Flotilla service listens on every address, so it can’t tell which one your phone
          reaches. Instead, {bind}.
        </p>
      ) : (
        <p>
          The Flotilla service listens only on this computer (loopback). To pair over Tailscale,{" "}
          {bind}.
        </p>
      )}
      <p>{BY_HAND}</p>
    </div>
  );
}

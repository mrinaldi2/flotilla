import type { ReactElement } from "react";
import type { NotifyLevel } from "../../protocol/entities";
import { errText } from "../../util";
import PairingCode from "./PairingCode";
import type { IssuedToken } from "./useDevices";

interface TokenRevealProps {
  readonly issued: IssuedToken;
  readonly onDismiss: () => void;
  readonly onToast: (level: NotifyLevel, title: string, body: string) => void;
}

/**
 * One-time device token: shown once, never returned by the daemon again,
 * with the code a phone pairs from. Dismissing it drops both.
 */
export default function TokenReveal({
  issued,
  onDismiss,
  onToast,
}: TokenRevealProps): ReactElement {
  // The toasts name what was copied, never the secret itself.
  const copy = async (text: string, title: string, body: string): Promise<void> => {
    try {
      await navigator.clipboard.writeText(text);
      onToast("info", title, body);
    } catch (error) {
      onToast("error", "Copy failed", errText(error));
    }
  };

  return (
    <div className="token-reveal">
      <p className="token-note">
        Token for “{issued.deviceName}” — copy it now, it will not be shown again.
      </p>
      <div className="token-box">
        <input
          readOnly
          aria-label="Device token"
          className="mono"
          value={issued.token}
          onFocus={(event) => {
            event.target.select();
          }}
        />
        <button
          type="button"
          className="btn btn-small"
          onClick={() => {
            void copy(issued.token, "Token copied", "The device token is on your clipboard.");
          }}
        >
          Copy token
        </button>
        <button type="button" className="btn btn-small" onClick={onDismiss}>
          Dismiss
        </button>
      </div>
      <PairingCode
        token={issued.token}
        pairing={issued.pairing}
        onCopyLink={(link) => {
          void copy(
            link,
            "Pairing link copied",
            "It holds the device token: paste it only on your phone.",
          );
        }}
      />
    </div>
  );
}

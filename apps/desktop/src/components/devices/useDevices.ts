import { useCallback, useState } from "react";
import { capture, captureException } from "../../analytics";
import { useLoadOnConnect } from "../../hooks/useLoadOnConnect";
import type { DaemonApi } from "../../protocol/api";
import type { Device, DeviceCapability, NotifyLevel } from "../../protocol/entities";
import { errText } from "../../util";
import { pairingTarget } from "./pairing";
import type { PairingTarget } from "./pairing";

/** Where the pairing code stands: the daemon's addresses load after the token. */
export type PairingState =
  | { readonly status: "loading" }
  | { readonly status: "failed"; readonly error: string }
  | { readonly status: "ready"; readonly target: PairingTarget };

/**
 * A just-created device's token, and what a phone needs to pair with it.
 * Held in memory only, until dismissed: never stored, logged or sent.
 */
export interface IssuedToken {
  readonly deviceName: string;
  readonly token: string;
  readonly pairing: PairingState;
}

export interface DevicesApi {
  /** Null until the first load completes. */
  readonly devices: readonly Device[] | null;
  readonly loadError: string | null;
  readonly issued: IssuedToken | null;
  readonly dismissIssued: () => void;
  readonly create: (name: string, capabilities: readonly DeviceCapability[]) => Promise<void>;
  readonly revoke: (deviceId: string) => Promise<void>;
}

type Toast = (level: NotifyLevel, title: string, body: string) => void;

export function useDevices(client: DaemonApi, connected: boolean, onToast: Toast): DevicesApi {
  const [devices, setDevices] = useState<readonly Device[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [issued, setIssued] = useState<IssuedToken | null>(null);

  const load = useCallback(async (): Promise<void> => {
    try {
      const reply = await client.request({ type: "list_devices" }, "devices");
      setDevices(reply.devices);
      setLoadError(null);
    } catch (error) {
      captureException(error, "device_list");
      setLoadError(errText(error));
    }
  }, [client]);

  useLoadOnConnect(connected, load);

  // The pairing link's addresses come from the daemon's config. Only the
  // token they were read for is updated: a newer device's token replaces it.
  const loadPairing = useCallback(
    async (token: string): Promise<void> => {
      let pairing: PairingState;
      try {
        const reply = await client.request({ type: "get_config" }, "config");
        pairing = { status: "ready", target: pairingTarget(reply.config) };
      } catch (error) {
        pairing = { status: "failed", error: errText(error) };
      }
      setIssued((prev) => (prev?.token === token ? { ...prev, pairing } : prev));
    },
    [client],
  );

  const create = useCallback(
    async (name: string, capabilities: readonly DeviceCapability[]): Promise<void> => {
      try {
        const reply = await client.request({ type: "create_device", name, capabilities }, "device");
        capture("device_created", { capabilities });
        setDevices((prev) => [reply.device, ...(prev ?? [])]);
        if (typeof reply.token === "string") {
          setIssued({
            deviceName: reply.device.name,
            token: reply.token,
            pairing: { status: "loading" },
          });
          void loadPairing(reply.token);
        } else {
          onToast("warn", "Device created", "The daemon did not return a token.");
        }
      } catch (error) {
        captureException(error, "device_create");
        onToast("error", "Create device failed", errText(error));
      }
    },
    [client, loadPairing, onToast],
  );

  const revoke = useCallback(
    async (deviceId: string): Promise<void> => {
      try {
        const reply = await client.request(
          { type: "revoke_device", device_id: deviceId },
          "device",
        );
        capture("device_revoked", {});
        setDevices((prev) =>
          (prev ?? []).map((device) => (device.id === reply.device.id ? reply.device : device)),
        );
        onToast("info", "Device revoked", `${reply.device.name} can no longer connect.`);
      } catch (error) {
        captureException(error, "device_revoke");
        onToast("error", "Revoke failed", errText(error));
      }
    },
    [client, onToast],
  );

  const dismissIssued = useCallback((): void => {
    setIssued(null);
  }, []);

  return { devices, loadError, issued, dismissIssued, create, revoke };
}

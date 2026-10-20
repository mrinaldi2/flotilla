// The team on demand in Settings (0.23.0): how many bots may run at once on
// this computer, and how long an on-demand bot stays idle before it sleeps.
// Both take effect at once; lowering the cap stops no running bot.

import { useState } from "react";
import type { ReactElement } from "react";
import type { DaemonApi } from "../../protocol/api";
import type { DaemonConfig, NotifyLevel } from "../../protocol/entities";
import { errText } from "../../util";

const RUNNING = { min: 1, max: 32, fallback: 6 } as const;
const IDLE = { min: 1, max: 1440, fallback: 10 } as const;

/** A whole number within the range, or null. */
function parseWithin(draft: string, range: { min: number; max: number }): number | null {
  const trimmed = draft.trim();
  if (!/^\d+$/.test(trimmed)) {
    return null;
  }
  const value = Number.parseInt(trimmed, 10);
  return value >= range.min && value <= range.max ? value : null;
}

interface TeamLimitsFormProps {
  readonly client: DaemonApi;
  readonly config: DaemonConfig;
  readonly connected: boolean;
  readonly canControl: boolean;
  readonly onSaved: (config: DaemonConfig) => void;
  readonly onToast: (level: NotifyLevel, title: string, body: string) => void;
}

/** A number and its unit: "6 bots". */
function NumberField(props: {
  readonly id: string;
  readonly value: string;
  readonly disabled: boolean;
  readonly unit: string;
  readonly onChange: (value: string) => void;
}): ReactElement {
  return (
    <>
      <input
        id={props.id}
        className="settings-number settings-port"
        inputMode="numeric"
        value={props.value}
        disabled={props.disabled}
        onChange={(event) => {
          props.onChange(event.target.value);
        }}
      />
      <span className="settings-unit">{props.unit}</span>
    </>
  );
}

export default function TeamLimitsForm(props: TeamLimitsFormProps): ReactElement {
  const { client, config, connected, canControl, onSaved, onToast } = props;
  const savedRunning = config.max_running_bots ?? RUNNING.fallback;
  const savedIdle = config.idle_sleep_minutes ?? IDLE.fallback;
  const [running, setRunning] = useState(String(savedRunning));
  const [idle, setIdle] = useState(String(savedIdle));
  const [saving, setSaving] = useState(false);
  const runningValue = parseWithin(running, RUNNING);
  const idleValue = parseWithin(idle, IDLE);
  const valid = runningValue !== null && idleValue !== null;
  const dirty = runningValue !== savedRunning || idleValue !== savedIdle;

  const save = async (): Promise<void> => {
    if (runningValue === null || idleValue === null) {
      return;
    }
    setSaving(true);
    try {
      const reply = await client.request(
        { type: "set_config", max_running_bots: runningValue, idle_sleep_minutes: idleValue },
        "config",
      );
      onSaved(reply.config);
      setRunning(String(reply.config.max_running_bots ?? runningValue));
      setIdle(String(reply.config.idle_sleep_minutes ?? idleValue));
      onToast("info", "Settings saved", "The running cap and idle time apply now.");
    } catch (error) {
      onToast("error", "Couldn't save the setting", errText(error));
    } finally {
      setSaving(false);
    }
  };

  return (
    <form
      className="settings-row settings-row-form"
      aria-label="Bots running at once"
      onSubmit={(event) => {
        event.preventDefault();
        void save();
      }}
    >
      <div className="settings-row-text">
        <label className="settings-row-label" htmlFor="settings-max-running">
          Bots running at once
        </label>
        <div className="settings-row-help">
          At most this many bots run on this computer, always-on ones included; a bot woken past it
          waits for a slot. 1–32, default 6.
        </div>
        <div className="settings-inline-fields">
          <NumberField
            id="settings-max-running"
            value={running}
            disabled={!canControl}
            unit="bots"
            onChange={setRunning}
          />
        </div>
        <label className="settings-row-label settings-row-next" htmlFor="settings-idle-minutes">
          Sleep after
        </label>
        <div className="settings-row-help">
          An on-demand bot with nothing to do sleeps after this many idle minutes. 1–1440, default
          10.
        </div>
        <div className="settings-inline-fields">
          <NumberField
            id="settings-idle-minutes"
            value={idle}
            disabled={!canControl}
            unit="minutes"
            onChange={setIdle}
          />
          {canControl ? (
            <button
              type="submit"
              className="btn btn-small btn-primary"
              disabled={!connected || saving || !dirty || !valid}
            >
              {saving ? "Saving…" : "Save"}
            </button>
          ) : null}
        </div>
        {valid ? null : (
          <div className="settings-row-error">Enter 1–32 bots and 1–1440 minutes.</div>
        )}
      </div>
    </form>
  );
}

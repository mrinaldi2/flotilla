import type { Story } from "@ladle/react";
import { useState } from "react";
import { teamDaemon } from "../../test/teamFixtures";
import TeamLimitsForm from "./TeamLimitsForm";

const noop = (): void => {};

/** The running cap and idle time, next to auto-compact (0.23.0). */
export const TeamLimits: Story = () => {
  const [client] = useState(teamDaemon);
  return (
    <div className="settings-section" style={{ maxWidth: 560, padding: 16 }}>
      <TeamLimitsForm
        client={client}
        config={{
          bind: ["127.0.0.1"],
          port: 49777,
          configured_port: 49777,
          runtime: "pty",
          auto_compact_window: null,
          max_running_bots: 6,
          idle_sleep_minutes: 10,
        }}
        connected
        canControl
        onSaved={noop}
        onToast={noop}
      />
    </div>
  );
};

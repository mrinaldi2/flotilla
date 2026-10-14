import type { ReactElement } from "react";
import type { Selection } from "../app/selection";
import { useBoardActivity } from "../app/useBoardActivity";
import { useBoardActivityPrefs } from "../prefs";
import type { DaemonApi } from "../protocol/api";
import type { Bot } from "../protocol/entities";
import ActivityToasts from "./ActivityToasts";

/** The app-wide board activity toasts, fed by the daemon's pushes (0.19.0). */
export default function BoardActivityLayer(props: {
  readonly client: DaemonApi;
  readonly bots: readonly Bot[];
  readonly select: (selection: Selection) => void;
}): ReactElement {
  const prefs = useBoardActivityPrefs();
  const activity = useBoardActivity({ ...props, prefs });
  return (
    <ActivityToasts
      toasts={activity.toasts}
      onOpen={activity.open}
      onDismiss={activity.dismiss}
      onHold={activity.hold}
    />
  );
}

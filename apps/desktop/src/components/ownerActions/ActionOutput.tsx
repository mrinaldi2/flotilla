// An owner action's output (H-117 R2): the redacted end once it ran, or
// what streams in while it runs; open while it runs and when it went wrong.

import type { ReactElement } from "react";
import type { OwnerAction } from "../../protocol/ownerActions";

export default function ActionOutput(props: {
  readonly action: OwnerAction;
  readonly live?: string;
}): ReactElement | null {
  const { state } = props.action;
  const text = props.live && state === "running" ? props.live : props.action.output_tail;
  if (!text) {
    return null;
  }
  // Open while it runs, and when it went wrong.
  const open = state === "running" || state === "failed" || state === "timed_out";
  return (
    <details className="owner-action-output" open={open}>
      <summary>Output</summary>
      <pre>{text}</pre>
    </details>
  );
}

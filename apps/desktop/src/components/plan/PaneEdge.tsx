// The edge the owner drags to widen or narrow the Plan tab's tree and its
// gate panel (0.22.0): a vertical separator between a pane and the page.

import type { ReactElement } from "react";
import type { PaneSpec, PaneWidth } from "../../hooks/usePaneWidth";

export const TREE_WIDTH: PaneSpec = {
  key: "plan.tree.width",
  initial: 360,
  min: 240,
  max: 720,
  pane: "before",
};

export const GATE_PANEL_WIDTH: PaneSpec = {
  key: "plan.gate.width",
  initial: 340,
  min: 280,
  max: 900,
  pane: "after",
};

export default function PaneEdge(props: {
  readonly label: string;
  readonly width: PaneWidth;
}): ReactElement {
  return (
    <div
      className="pane-edge"
      role="separator"
      aria-orientation="vertical"
      aria-label={props.label}
      tabIndex={0}
      title="Drag to resize · double-click to reset"
      {...props.width.edge}
    />
  );
}

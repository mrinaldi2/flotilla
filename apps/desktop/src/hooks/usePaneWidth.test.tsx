import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactElement } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import PaneEdge, { GATE_PANEL_WIDTH, TREE_WIDTH } from "../components/plan/PaneEdge";
import type { PaneSpec } from "./usePaneWidth";
import { usePaneWidth } from "./usePaneWidth";

function Panes(props: { readonly spec: PaneSpec }): ReactElement {
  const width = usePaneWidth(props.spec);
  return (
    <div>
      <aside aria-label="pane" style={width.style} />
      <PaneEdge label="Width" width={width} />
    </div>
  );
}

describe("a pane the owner resizes", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("grows toward the page with the arrow keys, remembers it, and resets", async () => {
    const user = userEvent.setup();
    const { unmount } = render(<Panes spec={TREE_WIDTH} />);
    const edge = screen.getByRole("separator", { name: "Width" });
    edge.focus();
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(edge).toHaveAttribute("aria-valuenow", "440");
    expect(screen.getByRole("complementary", { name: "pane" }).style.flexBasis).toBe("440px");
    unmount();
    render(<Panes spec={TREE_WIDTH} />);
    const again = screen.getByRole("separator", { name: "Width" });
    expect(again).toHaveAttribute("aria-valuenow", "440");
    await user.dblClick(again);
    expect(again).toHaveAttribute("aria-valuenow", String(TREE_WIDTH.initial));
  });

  it("grows a pane after the edge with the left arrow, within its bounds", async () => {
    const user = userEvent.setup();
    render(<Panes spec={GATE_PANEL_WIDTH} />);
    const edge = screen.getByRole("separator", { name: "Width" });
    edge.focus();
    await user.keyboard("{ArrowLeft}");
    expect(edge).toHaveAttribute("aria-valuenow", String(GATE_PANEL_WIDTH.initial + 40));
    await user.keyboard("{ArrowRight>10/}");
    expect(edge).toHaveAttribute("aria-valuenow", String(GATE_PANEL_WIDTH.min));
  });
});

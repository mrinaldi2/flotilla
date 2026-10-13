import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import type { ReactElement } from "react";
import { describe, expect, it } from "vitest";
import { useReturnFocus } from "./useReturnFocus";

function Dialog(props: { readonly onClose: () => void }): ReactElement {
  useReturnFocus();
  return (
    <div role="dialog" aria-label="Sheet">
      <input aria-label="Name" autoFocus />
      <button type="button" onClick={props.onClose}>
        Close
      </button>
    </div>
  );
}

function Opener(): ReactElement {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open
      </button>
      {open ? <Dialog onClose={() => setOpen(false)} /> : null}
    </>
  );
}

describe("useReturnFocus", () => {
  it("gives focus back to the opener once the dialog closes", async () => {
    const user = userEvent.setup();
    render(<Opener />);
    await user.click(screen.getByRole("button", { name: "Open" }));
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Close" }));
    expect(screen.getByRole("button", { name: "Open" })).toHaveFocus();
  });
});

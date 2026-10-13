// A dialog gives focus back to what opened it (UX-012): the element that
// had focus when it first rendered gets it again once the dialog is gone,
// unless focus has already gone somewhere else meanwhile.

import { useEffect, useState } from "react";

export function useReturnFocus(): void {
  // Read while rendering, before the dialog's own autofocus takes focus.
  const [opener] = useState(() =>
    document.activeElement instanceof HTMLElement ? document.activeElement : null,
  );
  useEffect(
    () => () => {
      const now = document.activeElement;
      if (opener?.isConnected === true && (now === null || now === document.body)) {
        opener.focus();
      }
    },
    [opener],
  );
}

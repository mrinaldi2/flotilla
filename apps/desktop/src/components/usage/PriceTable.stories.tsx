import type { Story } from "@ladle/react";
import { useState } from "react";
import type { ReactElement } from "react";
import { usageDaemon } from "../../test/usageFixtures";
import PriceTable from "./PriceTable";

const noop = (): void => {};

function Frame({ owner }: { readonly owner: boolean }): ReactElement {
  const [client] = useState(() => usageDaemon({ owner }));
  return (
    <div style={{ maxWidth: 760, padding: 16 }}>
      <PriceTable client={client} connected onSaved={noop} />
    </div>
  );
}

/** The owner edits Flotilla's seeded estimates: every cell a field, rows to add and remove. */
export const Editing: Story = () => <Frame owner />;

/** Without the owner's grant: the same table, read only. */
export const ReadOnly: Story = () => <Frame owner={false} />;

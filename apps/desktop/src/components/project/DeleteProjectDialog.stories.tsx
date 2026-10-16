import type { Story } from "@ladle/react";
import { useState } from "react";
import type { ReactElement } from "react";
import { FakeDaemon } from "../../test/fakeDaemon";
import { project } from "../../test/fixtures";
import { DELETE_PREVIEW } from "../../test/projectTrashFixtures";
import DeleteProjectDialog from "./DeleteProjectDialog";

const noop = (): void => {};

/** Deleting Acme: 1.6 GB to the Trash, by kind, and win told when it's back. */
function Frame({ light }: { readonly light?: boolean }): ReactElement {
  const [client] = useState(() =>
    new FakeDaemon().onRequest("project_delete_preview", () => ({
      type: "project_delete_preview",
      req_id: "1",
      preview: DELETE_PREVIEW,
    })),
  );
  return (
    <div className={`main ${light ? "theme-light" : ""}`}>
      <DeleteProjectDialog
        client={client}
        project={project()}
        botCount={3}
        onConfirm={noop}
        onCancel={noop}
      />
    </div>
  );
}

export const DeleteProject: Story = () => <Frame />;

export const DeleteProjectLight: Story = () => <Frame light />;

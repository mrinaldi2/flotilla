import type { Story } from "@ladle/react";
import type { ReactElement } from "react";
import { draftDetail } from "../../../test/boardsFixtures";
import { boardsClient } from "../../../test/boardStory";
import AttachmentsSection from "./AttachmentsSection";
import AttachmentViewer from "./AttachmentViewer";

const noop = (): void => {};
const linked = (): Promise<void> => Promise.resolve();
const attachments = draftDetail().attachments;

function Section(props: { readonly uploads?: boolean; readonly owner?: boolean }): ReactElement {
  return (
    <div className="item-drawer" style={{ position: "relative", height: 520 }}>
      <div className="drawer-body">
        <AttachmentsSection
          api={boardsClient()}
          attachments={attachments}
          uploads={
            props.uploads === true
              ? [
                  {
                    id: "u1",
                    name: "network-log.txt",
                    size: 2_400_000,
                    received: 1_048_576,
                    refusal: null,
                  },
                  {
                    id: "u2",
                    name: "IMG_0042.heic",
                    size: 3_100_000,
                    received: 0,
                    refusal: "HEIC isn't taken: export it as JPEG and attach that.",
                  },
                  {
                    id: "u3",
                    name: "screen-recording.png",
                    size: 14_000_000,
                    received: 0,
                    refusal: "13.4 MB is over the 10.0 MB cap for an image.",
                  },
                ]
              : []
          }
          actions={
            props.owner === false
              ? undefined
              : { onAddFiles: noop, onLink: linked, onDetach: noop, onDismiss: noop }
          }
          onOpen={noop}
        />
      </div>
    </div>
  );
}

/** An image thumbnail, a file, a linked document, and one detached. */
export const List: Story = () => <Section />;

/** Uploads: one on its way with its progress, two refused at once with the reason. */
export const Uploading: Story = () => <Section uploads />;

/** Read only: no Add, Link or Detach without the control grant. */
export const ReadOnly: Story = () => <Section owner={false} />;

/** A screenshot full size. */
export const Viewer: Story = () => {
  const shot = attachments.find((a) => a.id === "att-shot");
  return shot === undefined ? (
    <p />
  ) : (
    <div style={{ height: 600 }}>
      <AttachmentViewer api={boardsClient()} attachment={shot} onClose={noop} />
    </div>
  );
};

/** A linked document's details. */
/** A linked spec, read from the project's artifacts as formatted text (0.20.0). */
const SPEC = `# RB-20 · Import recipes from a link

The owner pastes a recipe page's address; the bot makes the card's recipe.

## Acceptance

- The title, the ingredients and the steps come from the page.
- A page it can't read says why, and nothing is created.
`;

/** A linked report: its page and the stylesheet beside it. */
const REPORT: Record<string, { readonly mime: string; readonly text: string }> = {
  "reports/import/index.html": {
    mime: "text/html",
    text: '<link rel="stylesheet" href="report.css"><h1>Import benchmark</h1><p>12 of 12 recipes imported.</p>',
  },
  "reports/import/report.css": {
    mime: "text/css",
    text: "body{font:15px sans-serif;margin:24px;color:#14161c}h1{color:#2a6f68}",
  },
};

const readArtifact = async (
  path: string,
): Promise<{ readonly mime: string; readonly text: string }> => {
  if (path === "specs/RB-20-spec.md") {
    return { mime: "text/markdown", text: SPEC };
  }
  const file = REPORT[path];
  if (file === undefined) {
    throw new Error(`no artifact ${path}`);
  }
  return file;
};

export const DocumentDetails: Story = () => {
  const spec = attachments.find((a) => a.id === "att-spec");
  return spec === undefined ? (
    <p />
  ) : (
    <div style={{ height: 600 }}>
      <AttachmentViewer
        api={boardsClient()}
        attachment={spec}
        readArtifact={readArtifact}
        onClose={noop}
      />
    </div>
  );
};

/** A linked HTML report in its sandbox, its stylesheet read beside it (0.20.0). */
export const DocumentPage: Story = () => {
  const spec = attachments.find((a) => a.id === "att-spec");
  return spec === undefined ? (
    <p />
  ) : (
    <div style={{ height: 600 }}>
      <AttachmentViewer
        api={boardsClient()}
        attachment={{
          ...spec,
          id: "att-report",
          name: "Import benchmark",
          path: "reports/import/index.html",
        }}
        readArtifact={readArtifact}
        onClose={noop}
      />
    </div>
  );
};

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
export const DocumentDetails: Story = () => {
  const spec = attachments.find((a) => a.id === "att-spec");
  return spec === undefined ? (
    <p />
  ) : (
    <div style={{ height: 600 }}>
      <AttachmentViewer api={boardsClient()} attachment={spec} onClose={noop} />
    </div>
  );
};

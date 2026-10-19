import type { Story } from "@ladle/react";
import { useRef, useState } from "react";
import type { ReactElement } from "react";
import { ARTIFACTS_V2 } from "../../protocol/chat";
import { GROUPED_ARTIFACTS } from "../../test/artifactFixtures";
import { FakeDaemon } from "../../test/fakeDaemon";
import { bot } from "../../test/fixtures";
import { useClickOnce } from "../../test/storyClick";
import FilesPanel from "./FilesPanel";

const noop = (): void => {};

function Panel(props: { readonly click?: string }): ReactElement {
  const box = useRef<HTMLDivElement>(null);
  const [client] = useState(() => {
    const fake = new FakeDaemon().onRequest("list_artifacts", (body) => {
      const group = body.type === "list_artifacts" ? (body.group ?? "shared") : "shared";
      return {
        type: "artifacts",
        req_id: "1",
        project_id: "p1",
        artifacts: GROUPED_ARTIFACTS.filter((a) => a.group === group),
        has_more: group === "shared",
        next_before: group === "shared" ? "2024-04-17T16:20:00Z|notes-0917.md" : null,
      };
    });
    fake.capabilities = [...fake.capabilities, ARTIFACTS_V2];
    fake.grants = ["read", "control", "approve"];
    return fake;
  });
  useClickOnce(box, props.click);
  return (
    <div ref={box} className="bot-info-panel" style={{ width: 340, height: 560 }}>
      <FilesPanel
        client={client}
        bot={bot()}
        connected
        selected={null}
        onSelect={noop}
        canControl
      />
    </div>
  );
}

/** Shared files, each with the cards that use it, and older ones a click away. */
export const Shared: Story = () => <Panel />;
/** The owner's uploads. */
export const Uploads: Story = () => <Panel click="Uploads" />;
/** Shared files no card uses or kept: keep, attach to a card, or remove. */
export const TidyUp: Story = () => <Panel click="Tidy up" />;

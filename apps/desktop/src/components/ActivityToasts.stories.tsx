import type { Story } from "@ladle/react";
import type { ActivityLine, ActivityToast } from "../app/boardActivity";
import ActivityToasts from "./ActivityToasts";

const noop = (): void => {};

type ToastInit = Partial<Omit<ActivityToast, "lines">> & {
  readonly lines: readonly ActivityLine[];
};

function toast({ lines, ...over }: ToastInit): ActivityToast {
  return {
    id: 1,
    key: "k",
    actorLabel: "New Bot",
    projectId: "p1",
    itemId: "P2-001",
    itemTitle: "Test the board toasts",
    lines: lines.map((line, i) => ({ ...line, id: i + 1 })),
    updatedAt: 0,
    ...over,
  };
}

const worked = toast({
  id: 3,
  lines: [
    { glyph: "●", word: "Started", detail: "working on it" },
    { glyph: "→", word: "Moved", detail: "To do → Doing" },
    { glyph: "✓", word: "Finished", detail: "a turn" },
  ],
});

const refused = toast({
  id: 2,
  actorLabel: "Desktop Dev",
  itemId: "P2-004",
  itemTitle: "Ship the release notes",
  lines: [
    {
      glyph: "⊘",
      word: "Couldn't move",
      detail: "to Done · Only the owner moves a card to Done on this board.",
    },
  ],
});

const automatic = toast({
  id: 1,
  actorLabel: "Automatic",
  itemId: "P2-002",
  itemTitle: "Fix the drawer focus",
  lines: [{ glyph: "→", word: "Moved", detail: "Review → Done · PR merged" }],
});

export const Stack: Story = () => (
  <ActivityToasts
    toasts={[worked, refused, automatic]}
    onOpen={noop}
    onDismiss={noop}
    onHold={noop}
  />
);

export const PickedUp: Story = () => (
  <ActivityToasts
    toasts={[toast({ lines: [{ glyph: "+", word: "Picked up", detail: "" }] })]}
    onOpen={noop}
    onDismiss={noop}
    onHold={noop}
  />
);

export const OtherDeviceAndDraft: Story = () => (
  <ActivityToasts
    toasts={[
      toast({
        id: 2,
        actorLabel: "You, on iPhone",
        itemId: "P2-007",
        itemTitle: "",
        lines: [{ glyph: "+", word: "Assigned", detail: "to New Bot" }],
      }),
      toast({
        id: 1,
        itemId: "P2-010",
        itemTitle:
          "A very long draft title that has to be cut off before it pushes the close button out",
        lines: [
          { glyph: "✎", word: "Updated the draft", detail: "description, acceptance criteria" },
        ],
      }),
    ]}
    onOpen={noop}
    onDismiss={noop}
    onHold={noop}
  />
);

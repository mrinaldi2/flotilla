// The drafts tray (0.19.0): "✎ 2 drafts" at the right of the board
// toolbar, listing the owner's open drafts with the bot each is being
// discussed with. Choosing one opens it in the drawer.

import { useEffect, useState } from "react";
import type { ReactElement } from "react";
import type { BoardApi } from "../../../protocol/board";
import { boardCall } from "../../../protocol/board";
import type { Bot } from "../../../protocol/entities";
import type { ItemCard } from "../../../protocol/gen/hermes/board/v1/board_pb";
import { typeLabel } from "../labels";
import MenuButton from "../MenuButton";

interface DraftsTrayProps {
  readonly api: BoardApi;
  readonly drafts: readonly ItemCard[];
  readonly bots: readonly Bot[];
  readonly onOpen: (itemId: string) => void;
  readonly initiallyOpen?: boolean;
}

/** Each draft's invited bots, read from the board's home. */
function useInvited(
  api: BoardApi,
  drafts: readonly ItemCard[],
): ReadonlyMap<string, readonly string[]> {
  const [invited, setInvited] = useState<ReadonlyMap<string, readonly string[]>>(new Map());
  const key = drafts.map((d) => `${d.id}@${d.version}@${d.invited.join("+")}`).join(",");
  // `key` names the drafts, their versions and invitations: a change reads
  // them again.
  useEffect(() => {
    let live = true;
    const ids = key === "" ? [] : key.split(",").map((part) => part.split("@")[0] ?? "");
    void Promise.all(
      ids.map(async (id): Promise<[string, readonly string[]]> =>
        boardCall(api, { case: "itemGet", value: { id } }, "item").then(
          (detail) => [id, detail.invited],
          () => [id, []],
        ),
      ),
    ).then((pairs) => live && setInvited(new Map(pairs)));
    return () => {
      live = false;
    };
  }, [api, key]);
  return invited;
}

export default function DraftsTray(props: DraftsTrayProps): ReactElement | null {
  const { drafts, bots } = props;
  const invited = useInvited(props.api, drafts);
  if (drafts.length === 0) {
    return null;
  }
  const names = (id: string): string =>
    (invited.get(id) ?? [])
      .map((b) => bots.find((bot) => bot.id === b)?.name ?? "a bot")
      .join(", ");
  return (
    <MenuButton
      className="drafts-tray"
      alignEnd
      label={
        <>
          <span aria-hidden="true">✎ </span>
          {`${drafts.length} ${drafts.length === 1 ? "draft" : "drafts"}`}
        </>
      }
      menuLabel="Drafts"
      initiallyOpen={props.initiallyOpen}
      entries={drafts.map((d) => {
        const who = names(d.id);
        const type = typeLabel(d.type);
        return {
          key: d.id,
          label: `${type.glyph} ${type.word} · ${d.id} ${d.title}`,
          note: who === "" ? "Not discussed yet" : `Discussing with ${who}`,
          onChoose: () => props.onOpen(d.id),
        };
      })}
    />
  );
}

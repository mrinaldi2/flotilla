// Create a bot with a role (0.24.0, `board_flow_v1`): a column assigns a
// role nobody holds, so the owner makes a bot named for the role and gives
// it the role in one go. Used by Board settings › Columns and by Needs you's
// "Nobody holds …" row.

import { useCallback, useState } from "react";
import type { AddToast } from "../../app/useToasts";
import type { DaemonApi } from "../../protocol/api";
import { boardCall } from "../../protocol/board";
import type { Bot } from "../../protocol/entities";
import { Role } from "../../protocol/gen/flotilla/board/v1/board_pb";
import type { BoardSnapshot } from "../../protocol/gen/flotilla/board/v1/requests_pb";
import { errText } from "../../util";
import { roleLabel } from "../bot/roleLabels";

/** A role as the daemon words it in a row's `role`: "architect", "reviewer.ux"… */
const TOKENS: ReadonlyMap<string, Role> = new Map([
  ["lead", Role.LEAD],
  ["coach", Role.COACH],
  ["devops", Role.DEVOPS],
  ["reviewer.arch", Role.REVIEWER_ARCH],
  ["reviewer.ux", Role.REVIEWER_UX],
  ["tester", Role.TESTER],
  ["dev", Role.DEV],
  ["reviewer.ce", Role.REVIEWER_CE],
  ["product", Role.PRODUCT],
  ["architect", Role.ARCHITECT],
]);

/** The role a token names; undefined for one this app doesn't know. */
export function roleOfToken(token: string | undefined): Role | undefined {
  return token === undefined ? undefined : TOKENS.get(token.trim().toLowerCase());
}

interface Created {
  readonly bot: Bot;
  /** The board after the role was given, when it was. */
  readonly snapshot: BoardSnapshot | null;
  /** Why the role wasn't given; the bot exists all the same. */
  readonly roleError: string | null;
}

/** `create_bot` named for the role, then `role_set` on the project's board. */
export async function createRoleBot(
  client: DaemonApi,
  projectId: string,
  role: Role,
): Promise<Created> {
  const reply = await client.request(
    { type: "create_bot", project_id: projectId, name: roleLabel(role) },
    "bot",
  );
  try {
    const snapshot = await boardCall(
      client,
      { case: "roleSet", value: { projectId, bot: reply.bot.id, role } },
      "board",
    );
    return { bot: reply.bot, snapshot, roleError: null };
  } catch (error) {
    return { bot: reply.bot, snapshot: null, roleError: errText(error) };
  }
}

export interface RoleBotCreator {
  readonly create: (role: Role) => void;
  /** The role being created, while it is. */
  readonly creating: Role | null;
}

/** Creates a role's bot and says how it went; `onCreated` hears of a bot with its role. */
export function useCreateRoleBot(
  client: DaemonApi,
  projectId: string,
  addToast: AddToast,
  onCreated?: (bot: Bot, snapshot: BoardSnapshot) => void,
): RoleBotCreator {
  const [creating, setCreating] = useState<Role | null>(null);
  const create = useCallback(
    (role: Role): void => {
      const label = roleLabel(role);
      setCreating(role);
      void (async () => {
        try {
          const made = await createRoleBot(client, projectId, role);
          if (made.snapshot === null) {
            addToast(
              "error",
              `Created ${made.bot.name}, but couldn't make it the ${label}`,
              made.roleError ?? "",
            );
          } else {
            addToast("info", `${made.bot.name} holds ${label} now`, "Its waiting cards go to it.");
            onCreated?.(made.bot, made.snapshot);
          }
        } catch (error) {
          addToast("error", `Couldn't create a ${label} bot`, errText(error));
        } finally {
          setCreating(null);
        }
      })();
    },
    [client, projectId, addToast, onCreated],
  );
  return { create, creating };
}

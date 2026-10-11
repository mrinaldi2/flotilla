// The bot's board roles on its page (H-326): what it does on the project's
// board, and for the owner the controls to give or take a role away. Lead,
// DevOps, tester and reviewer.ce are the owner's alone (H-099, H-268): the
// daemon refuses them to the lead over MCP, so this is where they are set.

import { X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import type { FormEvent, ReactElement } from "react";
import { useLoadOnConnect } from "../../hooks/useLoadOnConnect";
import type { DaemonApi } from "../../protocol/api";
import { boardCall } from "../../protocol/board";
import type { Bot } from "../../protocol/entities";
import { Role } from "../../protocol/gen/hermes/board/v1/board_pb";
import type { ProjectRole } from "../../protocol/gen/hermes/board/v1/board_pb";
import { errText } from "../../util";

/** Each role as the owner reads it, in the order the picker lists them. */
const ROLE_LABEL: ReadonlyMap<Role, string> = new Map([
  [Role.LEAD, "Lead"],
  [Role.DEVOPS, "DevOps"],
  [Role.DEV, "Developer"],
  [Role.TESTER, "Tester"],
  [Role.REVIEWER_ARCH, "Reviewer · architecture"],
  [Role.REVIEWER_UX, "Reviewer · UX"],
  [Role.REVIEWER_CE, "Reviewer · security (ce)"],
  [Role.COACH, "Coach"],
]);

function roleLabel(role: Role): string {
  return ROLE_LABEL.get(role) ?? "Unknown role";
}

/** "Tester · imac": a tester says which machine it verifies on. */
function chipText(role: ProjectRole): string {
  const machine = role.machine?.trim();
  return role.role === Role.TESTER && machine !== undefined && machine !== ""
    ? `${roleLabel(role.role)} · ${machine}`
    : roleLabel(role.role);
}

interface Loaded {
  readonly roles: readonly ProjectRole[];
  /** The owner's own credentials on the board's home: may set roles here. */
  readonly canSet: boolean;
}

interface BotRolesProps {
  readonly client: DaemonApi;
  readonly bot: Bot;
  readonly connected: boolean;
}

/** The bot's roles, refetched whenever its project's board changes. */
function useBotRoles(
  client: DaemonApi,
  bot: Bot,
  connected: boolean,
): {
  readonly loaded: Loaded | null;
  readonly loadError: string | null;
  readonly setLoaded: (loaded: Loaded) => void;
} {
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const projectId = bot.project_id;

  const fetchRoles = useCallback(async (): Promise<void> => {
    try {
      const snap = await boardCall(client, { case: "boardGet", value: { projectId } }, "board");
      setLoaded({ roles: snap.roles, canSet: snap.canRule });
      setLoadError(null);
    } catch (error) {
      setLoadError(errText(error));
    }
  }, [client, projectId]);

  useLoadOnConnect(connected, fetchRoles);
  useEffect(
    () =>
      client.onBoardEvent((event) => {
        if (event.projectId === projectId) {
          void fetchRoles();
        }
      }),
    [client, projectId, fetchRoles],
  );

  return { loaded, loadError, setLoaded };
}

/** Sends one role change; resolves whether the daemon took it. */
type SendRole = (role: Role, remove: boolean, machine?: string) => Promise<boolean>;

/** The bot's roles as chips; ✕ takes one away when the owner may. */
function RoleChips(props: {
  readonly botName: string;
  readonly mine: readonly ProjectRole[];
  readonly canSet: boolean;
  readonly busy: boolean;
  readonly send: SendRole;
}): ReactElement {
  const { botName, mine, canSet, busy, send } = props;
  return (
    <ul className="bot-roles-list" aria-label={`${botName}'s board roles`}>
      {mine.length === 0 ? <li className="field-hint">No role on the board.</li> : null}
      {mine.map((r) => (
        <li key={r.role} className="bot-role-chip">
          <span>{chipText(r)}</span>
          {canSet ? (
            <button
              type="button"
              className="bot-role-remove"
              aria-label={`Remove ${roleLabel(r.role)}`}
              title={`Remove ${roleLabel(r.role)}`}
              disabled={busy}
              onClick={() => void send(r.role, true)}
            >
              <X size={12} aria-hidden="true" />
            </button>
          ) : null}
        </li>
      ))}
    </ul>
  );
}

/** The owner's picker: a role, a machine when it is tester, and Add. */
function AddRole(props: {
  readonly taken: readonly Role[];
  readonly busy: boolean;
  readonly send: SendRole;
  readonly onError: (error: string | null) => void;
}): ReactElement {
  const { taken, busy, send, onError } = props;
  const [role, setRole] = useState<Role>(Role.REVIEWER_CE);
  const [machine, setMachine] = useState("");
  const tester = role === Role.TESTER;

  const add = (event: FormEvent): void => {
    event.preventDefault();
    const where = machine.trim();
    if (tester && where === "") {
      onError("Say which machine the tester verifies on, such as imac or ios.");
      return;
    }
    void (async () => {
      if (await send(role, false, tester ? where : undefined)) {
        setMachine("");
      }
    })();
  };

  const pick = (value: string): void => {
    const picked = [...ROLE_LABEL.keys()].find((r) => String(r) === value);
    if (picked !== undefined) {
      setRole(picked);
    }
    onError(null);
  };

  return (
    <form className="bot-roles-add" onSubmit={add} noValidate>
      <label>
        <span className="visually-hidden">Role to add</span>
        <select
          value={role}
          disabled={busy}
          onChange={(event) => {
            pick(event.target.value);
          }}
        >
          {[...ROLE_LABEL].map(([value, label]) => (
            <option key={value} value={value} disabled={taken.includes(value)}>
              {label}
            </option>
          ))}
        </select>
      </label>
      {tester ? (
        <label>
          <span className="visually-hidden">Tester machine</span>
          <input
            type="text"
            placeholder="Machine, e.g. imac"
            value={machine}
            disabled={busy}
            onChange={(event) => {
              setMachine(event.target.value);
            }}
          />
        </label>
      ) : null}
      <button type="submit" className="btn btn-small" disabled={busy}>
        Add role
      </button>
    </form>
  );
}

/** Board roles: chips with ✕ for the owner, and a picker to add one. */
export default function BotRoles({ client, bot, connected }: BotRolesProps): ReactElement {
  const { loaded, loadError, setLoaded } = useBotRoles(client, bot, connected);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mine = (loaded?.roles ?? []).filter((r) => r.botId === bot.id);
  const canSet = loaded?.canSet === true && connected;

  const send: SendRole = async (role, remove, machine) => {
    setBusy(true);
    setError(null);
    try {
      const value = {
        projectId: bot.project_id,
        bot: bot.id,
        role,
        ...(machine === undefined ? {} : { machine }),
        ...(remove ? { remove: true } : {}),
      };
      const snap = await boardCall(client, { case: "roleSet", value }, "board");
      setLoaded({ roles: snap.roles, canSet: snap.canRule });
      return true;
    } catch (refusal) {
      setError(errText(refusal));
      return false;
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="bot-roles" aria-labelledby="bot-roles-title">
      <h4 className="bot-roles-title" id="bot-roles-title">
        Board roles
      </h4>
      {loadError !== null && loaded === null ? (
        <p className="field-hint">Roles are kept on the project&apos;s board: {loadError}</p>
      ) : null}
      {loaded === null ? null : (
        <RoleChips botName={bot.name} mine={mine} canSet={canSet} busy={busy} send={send} />
      )}
      {canSet ? (
        <AddRole taken={mine.map((r) => r.role)} busy={busy} send={send} onError={setError} />
      ) : null}
      {loaded !== null && !loaded.canSet ? (
        <p className="field-hint">
          Only the owner sets roles, from the app on the board&apos;s home computer.
        </p>
      ) : null}
      {error === null ? null : (
        <p className="field-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}

// The board's roles as the owner reads them (H-326), shared by a bot's
// Board roles and a card's "Assign to role…". Product and Architect are
// 0.23.0's (`team_on_demand_v1`): an older service refuses them.

import { Role } from "../../protocol/gen/flotilla/board/v1/board_pb";

/** Each role, in the order the pickers list them. */
const ROLE_LABEL: ReadonlyMap<Role, string> = new Map([
  [Role.LEAD, "Lead"],
  [Role.PRODUCT, "Product"],
  [Role.ARCHITECT, "Architect"],
  [Role.DEVOPS, "DevOps"],
  [Role.DEV, "Developer"],
  [Role.TESTER, "Tester"],
  [Role.REVIEWER_ARCH, "Reviewer · architecture"],
  [Role.REVIEWER_UX, "Reviewer · UX"],
  [Role.REVIEWER_CE, "Reviewer · security (ce)"],
  [Role.COACH, "Coach"],
]);

/** The roles 0.23.0 added. */
const NEW_ROLES: ReadonlySet<Role> = new Set([Role.PRODUCT, Role.ARCHITECT]);

/** A card goes to the role that owns its next step; never to the lead or the coach. */
const NOT_ASSIGNED: ReadonlySet<Role> = new Set([Role.LEAD, Role.COACH]);

export function roleLabel(role: Role): string {
  return ROLE_LABEL.get(role) ?? "Unknown role";
}

/** The roles this service knows, with their words, in picker order. */
export function rolesFor(teamOnDemand: boolean): readonly (readonly [Role, string])[] {
  return [...ROLE_LABEL].filter(([role]) => teamOnDemand || !NEW_ROLES.has(role));
}

/** The roles a card may be assigned to (0.23.0). */
export function assignableRoles(): readonly (readonly [Role, string])[] {
  return rolesFor(true).filter(([role]) => !NOT_ASSIGNED.has(role));
}

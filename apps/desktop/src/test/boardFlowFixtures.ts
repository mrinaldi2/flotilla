// The board runs the SDLC (0.24.0, `board_flow_v1`) for stories and tests:
// Recipes' columns with the role each assigns on entry, and who holds them.

import { create } from "@bufbuild/protobuf";
import {
  type BoardColumn,
  BoardColumnSchema,
  type ProjectRole,
  ProjectRoleSchema,
  Role,
} from "../protocol/gen/flotilla/board/v1/board_pb";
import { recipeColumns } from "./boardsFixtures";

/** Recipes as the board runs the SDLC (0.24.0): To do → Product, Doing → Developer, QA → Tester. */
export function flowColumns(): BoardColumn[] {
  const onEntry: Readonly<Record<string, Role>> = {
    "rb-todo": Role.PRODUCT,
    "rb-doing": Role.DEV,
    "rb-qa": Role.TESTER,
  };
  return recipeColumns().map((c) =>
    create(BoardColumnSchema, { ...c, onEntry: onEntry[c.key] ?? Role.UNSPECIFIED }),
  );
}

/** Who holds the roles: b1 and b3 are Developers, b2 the Tester; nobody is Product. */
export function flowRoles(): ProjectRole[] {
  const held: readonly (readonly [Role, string])[] = [
    [Role.LEAD, "lead"],
    [Role.DEV, "b1"],
    [Role.DEV, "b3"],
    [Role.TESTER, "b2"],
  ];
  return held.map(([role, botId]) => create(ProjectRoleSchema, { projectId: "p1", role, botId }));
}

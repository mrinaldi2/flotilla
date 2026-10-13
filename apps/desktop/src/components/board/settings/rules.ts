// A column's movers and entry rules as the board's tokens (`lead`,
// `roles:dev,tester`, `size_at_most:M`, `fields_filled:Goal;Steps`), in
// words for the settings panel, and the kinds and facts the panel offers.
// The Hermes service parses the same tokens and refuses any it doesn't know.

import { ColumnKind } from "../../../protocol/gen/hermes/board/v1/board_pb";

/** A kind chip: a glyph and a word, never colour alone. */
export const KINDS: readonly {
  readonly kind: ColumnKind;
  readonly glyph: string;
  readonly word: string;
}[] = [
  { kind: ColumnKind.OPEN, glyph: "○", word: "open" },
  { kind: ColumnKind.ACTIVE, glyph: "◐", word: "active" },
  { kind: ColumnKind.DONE, glyph: "●", word: "done" },
  { kind: ColumnKind.CANCELLED, glyph: "⊘", word: "cancelled" },
];

export function kindChip(kind: ColumnKind): string {
  const found = KINDS.find((k) => k.kind === kind);
  return found === undefined ? "◌ unset" : `${found.glyph} ${found.word}`;
}

/** Who may move an item into a column; none set means any member. */
export const MOVERS: readonly { readonly token: string; readonly word: string }[] = [
  { token: "any_member", word: "Any member" },
  { token: "assignee", word: "The assignee" },
  { token: "lead", word: "The lead" },
  { token: "owner", word: "You" },
  { token: "system", word: "Automations only" },
];

/** The board roles a `roles:` mover may name. */
export const ROLES: readonly string[] = [
  "lead",
  "dev",
  "tester",
  "devops",
  "coach",
  "reviewer.arch",
  "reviewer.ux",
  "reviewer.ce",
];

/** What a rule's parameter is: none, one choice, or free text. */
type Param =
  | { readonly kind: "none" }
  | { readonly kind: "choice"; readonly options: readonly string[] }
  | { readonly kind: "text"; readonly hint: string };

interface RuleSpec {
  readonly head: string;
  readonly word: string;
  readonly param: Param;
}

const NONE: Param = { kind: "none" };

/** The rule library (kanban-rework.md), in the order the panel lists it. */
export const RULES: readonly RuleSpec[] = [
  { head: "assignee_set", word: "Has an assignee", param: NONE },
  {
    head: "fields_filled",
    word: "Template sections filled",
    param: { kind: "text", hint: "Sections, separated by ;" },
  },
  { head: "ac_present", word: "Has acceptance criteria", param: NONE },
  { head: "ac_all_checked", word: "All criteria checked", param: NONE },
  {
    head: "size_at_most",
    word: "Size at most",
    param: { kind: "choice", options: ["S", "M", "L"] },
  },
  { head: "blockers_done", word: "Blocking items are done", param: NONE },
  { head: "not_blocked", word: "Not flagged blocked", param: NONE },
  {
    head: "link",
    word: "Has a link of kind",
    param: { kind: "choice", options: ["pr", "branch", "artifact", "task", "decision", "meeting"] },
  },
  { head: "independent", word: "Moved by someone other than the author", param: NONE },
  { head: "reason", word: "The move gives a reason", param: NONE },
  {
    head: "predicate",
    word: "Module check",
    param: { kind: "choice", options: ["code.linked", "release.in_open_package"] },
  },
];

/** "size_at_most:M" → head "size_at_most", arg "M". */
export function splitToken(token: string): { readonly head: string; readonly arg: string | null } {
  const at = token.indexOf(":");
  return at < 0
    ? { head: token.trim(), arg: null }
    : { head: token.slice(0, at).trim(), arg: token.slice(at + 1).trim() };
}

export function joinToken(head: string, arg: string | null): string {
  return arg === null ? head : `${head}:${arg}`;
}

export function ruleSpec(head: string): RuleSpec | undefined {
  return RULES.find((rule) => rule.head === head);
}

/** The first parameter a rule takes when switched on. */
export function defaultArg(spec: RuleSpec): string | null {
  switch (spec.param.kind) {
    case "none":
      return null;
    case "choice":
      return spec.param.options[0] ?? "";
    case "text":
      return "";
  }
}

/** A rule in words: "Size at most M", "Has a link of kind pr". */
export function ruleWords(token: string): string {
  const { head, arg } = splitToken(token);
  const spec = ruleSpec(head);
  if (spec === undefined) {
    return token;
  }
  if (arg === null || arg === "") {
    return spec.word;
  }
  return head === "fields_filled"
    ? `${spec.word}: ${arg.split(";").join(", ")}`
    : `${spec.word} ${arg}`;
}

/** Movers in words: "The lead, the assignee"; none set, any member. */
export function moverWords(tokens: readonly string[]): string {
  if (tokens.length === 0) {
    return "Any member";
  }
  return tokens
    .map((token) => {
      const { head, arg } = splitToken(token);
      if (head === "roles") {
        return `Roles ${(arg ?? "").split(",").join(", ")}`;
      }
      return MOVERS.find((m) => m.token === head)?.word ?? token;
    })
    .join(", ");
}

/** The roles a `roles:` token names. */
export function rolesOf(tokens: readonly string[]): string[] {
  const found = tokens.map(splitToken).find((t) => t.head === "roles");
  return found?.arg ? found.arg.split(",").filter(Boolean) : [];
}

/** `tokens` with the roles mover set to `roles` (dropped when empty). */
export function withRoles(tokens: readonly string[], roles: readonly string[]): string[] {
  const rest = tokens.filter((t) => splitToken(t).head !== "roles");
  return roles.length === 0 ? rest : [...rest, joinToken("roles", roles.join(","))];
}

/** `tokens` with `head` switched on (with `arg`) or off. */
export function toggledRule(
  tokens: readonly string[],
  head: string,
  on: boolean,
  arg: string | null,
): string[] {
  const rest = tokens.filter((t) => splitToken(t).head !== head);
  return on ? [...rest, joinToken(head, arg)] : rest;
}

/** The facts the modules register (kanban/facts.rs), as the owner reads them. */
export const FACTS: readonly { readonly name: string; readonly word: string }[] = [
  { name: "pr.opened", word: "PR opened" },
  { name: "pr.merged", word: "PR merged" },
  { name: "pr.closed", word: "PR closed without merging" },
  { name: "pr.changes_requested", word: "Owner asked for changes on the PR" },
  { name: "release.submitted", word: "Submitted in a release package" },
  { name: "release.withdrawn", word: "Taken out of a release package" },
  { name: "release.approved", word: "Release approved to ship" },
  { name: "release.held", word: "Held by the owner's release verdict" },
  { name: "release.rework", word: "Sent back by the owner's release verdict" },
  { name: "release.deployed", word: "Deployed" },
  { name: "release.deploy_failed", word: "Deploy failed" },
  { name: "release.left_out_reverted", word: "Left out of a release and reverted" },
];

export function factWord(name: string): string {
  return FACTS.find((fact) => fact.name === name)?.word ?? name;
}

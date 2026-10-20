# Lighter checkouts (0.23.0)

Bots used to make their own git worktrees, each with its own build output, so
a busy project filled the disk. From 0.23 a project with a repository keeps
one shared copy of it, gives a worktree only to the card that needs one, and
shares build caches across the computer. The ideas come from Anthropic's
[Scion](https://www.anthropic.com/engineering/redesigning-source-control),
adapted to one computer and plain git: one shared store, partial working
copies only for those who write, and shared, bounded caches. The wire
contract is in [protocol.md](protocol.md#the-team-on-demand-and-lighter-checkouts-0230)
(capability `repo_mirror_v1`).

## The project's mirror

The daemon keeps one bare mirror of the project's repository at
`~/.flotilla/repos/<project>.git`, fetched on a timer and before each new
worktree. Every worktree of the project shares its objects. A project set to
a branch the repository doesn't have reads the repository's default branch.

## A worktree per card

A bot that writes code asks for its card's worktree with `worktree_for
{item_id, paths?}`. The daemon makes it from the mirror on the card's branch,
in `<project>/worktrees/<card>`, one per card and shared by every bot on it.
With `paths`, the checkout is sparse: only those folders; a later ask can
widen it. When the card's pull request merges, the cleanup after merge
removes the worktree; a cancelled card's worktree is salvaged first.

## Reading without a worktree

Bots that only read (Product, Architect, reviewers, a Tester that doesn't run
the code) don't need a worktree. They use, against the mirror at any branch
or commit:

| Tool | Reads |
|---|---|
| `repo_file {path, ref?}` | one file |
| `repo_diff {base, head?, path?}` | the diff between two commits |
| `repo_log {ref?, path?, limit?}` | the history |
| `repo_grep {pattern, ref?, path?}` | matches across the tree |

## Shared build caches

- **Rust:** builds go through one `sccache` under `~/.flotilla/cache/sccache`
  when `sccache` is installed (`brew install sccache`); without it nothing
  changes.
- **Node:** pnpm installs share one content store under
  `~/.flotilla/cache/pnpm-store`, hard-linked into each worktree.
- **Warm seeds:** on an APFS disk, a new card worktree starts with the
  `node_modules` of its newest sibling, copied with `clonefile`, so unchanged
  files cost no extra disk.

A variable a bot's session already sets wins over these.

## The disk budget

A project's **Disk budget**, in its settings (owner only), caps what its worktrees
and caches may use. Over budget, the hourly disk report removes the
worktrees of merged cards first and raises a Needs-you row for the rest.

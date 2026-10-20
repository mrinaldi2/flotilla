//! Lighter checkouts (0.23.0, `repo_mirror_v1`; docs/protocol.md "The team
//! on demand and lighter checkouts"): one mirror per project repository
//! ([`mirror`]), a worktree per card made from it ([`card`]), read-only
//! repository tools that need no worktree ([`read`]), warm seeds for new
//! worktrees ([`seed`]), and the project's disk budget ([`budget`]).
//! Shared build caches are set beside each bot's Cargo target
//! (`workers::target`).

pub mod budget;
pub mod card;
pub mod mirror;
pub mod read;
pub mod seed;
pub mod tidy;

pub use tidy::spawn;

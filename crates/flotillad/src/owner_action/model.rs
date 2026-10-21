//! An owner action's own types (H-117 R1).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// How the content runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shell {
    Zsh,
    Bash,
    Powershell,
    Cmd,
}

impl Shell {
    pub const ALL: [Shell; 4] = [Shell::Zsh, Shell::Bash, Shell::Powershell, Shell::Cmd];

    pub fn as_str(self) -> &'static str {
        match self {
            Shell::Zsh => "zsh",
            Shell::Bash => "bash",
            Shell::Powershell => "powershell",
            Shell::Cmd => "cmd",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == text)
    }

    /// This computer's default: zsh on macOS, bash on Linux and other Unix
    /// systems (which may not have zsh), PowerShell on Windows.
    pub fn here() -> Self {
        if cfg!(windows) {
            Shell::Powershell
        } else if cfg!(target_os = "macos") {
            Shell::Zsh
        } else {
            Shell::Bash
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Proposed,
    Running,
    Succeeded,
    Failed,
    TimedOut,
    Rejected,
    Withdrawn,
    Expired,
}

impl State {
    pub const ALL: [State; 8] = [
        State::Proposed,
        State::Running,
        State::Succeeded,
        State::Failed,
        State::TimedOut,
        State::Rejected,
        State::Withdrawn,
        State::Expired,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            State::Proposed => "proposed",
            State::Running => "running",
            State::Succeeded => "succeeded",
            State::Failed => "failed",
            State::TimedOut => "timed_out",
            State::Rejected => "rejected",
            State::Withdrawn => "withdrawn",
            State::Expired => "expired",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == text)
    }

    pub fn is_open(self) -> bool {
        self == State::Proposed
    }

    /// How far along it is: proposed, running, then ended. A copy that
    /// follows another never goes back a stage.
    pub fn stage(self) -> u8 {
        match self {
            State::Proposed => 0,
            State::Running => 1,
            _ => 2,
        }
    }
}

/// A file the content relies on, hashed at proposal and again at run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pinned {
    pub path: String,
    pub sha256: String,
}

/// What a push request pushes (0.24.0, `push_requests_v1`): read from the
/// bot's worktree when it asked. The content is exactly
/// `git push origin <branch>:<target>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushDetails {
    pub branch: String,
    pub target: String,
    /// "owner/name", or the origin's URL when it names no such pair.
    pub repo: String,
    /// The branch's head when the bot asked: a run of a branch that moved
    /// since is refused.
    pub commit: String,
    pub files: u32,
    pub additions: u32,
    pub deletions: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
}

/// What a bot proposes: every field the sha256 covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    pub project_id: String,
    /// `bot:<id>`, or `daemon` for a template the daemon filled (R4).
    pub proposed_by: String,
    pub item_id: Option<String>,
    pub decision_id: Option<String>,
    /// The machine name it runs on: this daemon's, or a peer's.
    pub target_machine: String,
    pub shell: Shell,
    pub cwd: String,
    pub content: String,
    pub pinned_files: Vec<Pinned>,
    pub reason: String,
    pub timeout_s: u32,
    /// A push request's details (`kind: "push"`); `None` for a command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push: Option<PushDetails>,
}

impl Proposal {
    /// `push` for a push request, `command` for every other action.
    pub fn kind(&self) -> &'static str {
        match self.push {
            Some(_) => "push",
            None => "command",
        }
    }

    /// The sha256 over the canonical JSON of every field: object keys sorted
    /// (serde_json's map is ordered), no whitespace. What the owner's client
    /// shows and sends back. A push request's kind and details are covered
    /// too; a command's hash is what it was before they existed.
    pub fn sha256(&self) -> String {
        let mut canonical = json!({
            "project_id": self.project_id,
            "proposed_by": self.proposed_by,
            "item_id": self.item_id,
            "decision_id": self.decision_id,
            "target_machine": self.target_machine,
            "shell": self.shell.as_str(),
            "cwd": self.cwd,
            "content": self.content,
            "pinned_files": self.pinned_files,
            "reason": self.reason,
            "timeout_s": self.timeout_s,
        });
        if let Some(push) = &self.push {
            canonical["kind"] = json!(self.kind());
            canonical["push"] = json!(push);
        }
        hex::encode(Sha256::digest(canonical.to_string().as_bytes()))
    }
}

/// One owner action as stored.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnerAction {
    pub id: String,
    pub proposal: Proposal,
    pub sha256: String,
    /// Warnings shown on the card, e.g. a path a bot can write that isn't
    /// pinned.
    pub flags: Vec<String>,
    /// `bot`, `daemon` (R4), or `peer:<id>` for a copy a peer offered (R3).
    pub origin: String,
    pub state: State,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub run_by: Option<String>,
    pub run_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub output_path: Option<String>,
    /// The last 64 KB of output, redacted.
    pub output_tail: Option<String>,
    pub reject_reason: Option<String>,
    /// On the target, for a peer's offer (R3): this daemon's linked project.
    /// `proposal.project_id` stays the proposer's, as the hash covers it.
    pub local_project_id: Option<String>,
}

impl OwnerAction {
    /// The project it belongs to on this computer.
    pub fn project_here(&self) -> &str {
        self.local_project_id
            .as_deref()
            .unwrap_or(&self.proposal.project_id)
    }

    /// The peer that offered this copy, on the computer it runs on (R3).
    pub fn offered_by(&self) -> Option<&str> {
        self.origin.strip_prefix("peer:")
    }

    /// What clients and bots read: never the full output, only its
    /// redacted tail.
    pub fn to_json(&self) -> Value {
        let mut v = self.wire();
        v["project_id"] = json!(self.project_here());
        v
    }

    /// Every field as the proposer hashed it, for the peer link (R3).
    pub fn wire(&self) -> Value {
        let p = &self.proposal;
        let mut v = json!({
            "id": self.id, "project_id": p.project_id, "proposed_by": p.proposed_by,
            "item_id": p.item_id, "decision_id": p.decision_id,
            "target_machine": p.target_machine, "shell": p.shell.as_str(), "cwd": p.cwd,
            "content": p.content, "pinned_files": p.pinned_files, "reason": p.reason,
            "timeout_s": p.timeout_s, "sha256": self.sha256, "flags": self.flags,
            "origin": self.origin, "state": self.state.as_str(),
            "created_at": self.created_at, "expires_at": self.expires_at,
            "run_by": self.run_by, "run_at": self.run_at, "finished_at": self.finished_at,
            "exit_code": self.exit_code, "output_tail": self.output_tail,
            "reject_reason": self.reject_reason,
            "kind": p.kind(),
        });
        if let Some(push) = &p.push {
            v["push"] = json!(push);
        }
        v
    }
}

/// An action as the peer link carries it ([`OwnerAction::wire`]).
#[derive(Deserialize)]
struct Wire {
    id: String,
    #[serde(flatten)]
    proposal: Proposal,
    sha256: String,
    #[serde(default)]
    flags: Vec<String>,
    origin: String,
    state: State,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    run_by: Option<String>,
    run_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    exit_code: Option<i32>,
    output_tail: Option<String>,
    reject_reason: Option<String>,
}

impl OwnerAction {
    /// Reads a peer's copy. Its hash must be the one its fields give, or
    /// it is refused: what a client shows is always what would run.
    pub fn from_wire(value: &Value) -> anyhow::Result<OwnerAction> {
        let w: Wire = serde_json::from_value(value.clone())?;
        anyhow::ensure!(
            w.proposal.sha256() == w.sha256,
            "the linked computer's copy of owner action {} doesn't match its hash",
            w.id
        );
        Ok(OwnerAction {
            id: w.id,
            proposal: w.proposal,
            sha256: w.sha256,
            flags: w.flags,
            origin: w.origin,
            state: w.state,
            created_at: w.created_at,
            expires_at: w.expires_at,
            run_by: w.run_by,
            run_at: w.run_at,
            finished_at: w.finished_at,
            exit_code: w.exit_code,
            output_path: None,
            output_tail: w.output_tail,
            reject_reason: w.reject_reason,
            local_project_id: None,
        })
    }
}

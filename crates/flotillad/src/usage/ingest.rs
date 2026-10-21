//! Reading a bot's transcripts into its usage: every Claude Code session
//! file of its workspace (subagents' too), or the Codex observation
//! record, each from where the last read stopped.

use std::path::{Path, PathBuf};

use bus::{Bot, BotRuntime};
use chrono::{DateTime, Local, Utc};

use super::parse::{self, Line};
use crate::app::AppState;
use crate::db::UsageEntry;

/// How deep under the workspace's transcript folder session files are
/// looked for: Claude Code keeps subagents' in `<session>/subagents/`.
const DEPTH: usize = 3;

/// What a read found.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Read {
    /// A count changed.
    pub changed: bool,
    /// The newest thing the runtime said was a rate limit, and when it
    /// resets if it said.
    pub limited: Option<(DateTime<Utc>, Option<DateTime<Utc>>)>,
}

/// The local date of `at`, as days are keyed.
pub fn day(at: DateTime<Utc>) -> String {
    at.with_timezone(&Local).date_naive().to_string()
}

/// The transcript files the bot's runtime writes.
pub fn files(app: &AppState, bot: &Bot) -> Vec<PathBuf> {
    if bot.is_linked() {
        return Vec::new();
    }
    let workspace = Path::new(&bot.workspace_path);
    match bot.runtime {
        BotRuntime::CodexCli => {
            let path = crate::runtime::codex::transcript_path(workspace);
            if path.exists() {
                vec![path]
            } else {
                Vec::new()
            }
        }
        BotRuntime::ClaudeCode => {
            let mut out = Vec::new();
            let dir = crate::activity::transcript_dir(&app.cfg.user_home, workspace);
            collect(&dir, DEPTH, &mut out);
            out.sort();
            out
        }
    }
}

fn collect(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() && depth > 1 {
            collect(&path, depth - 1, out);
        } else if kind.is_file() && path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
}

/// Reads what the bot's transcripts added since the last read into its
/// days, counted to its project.
pub fn read(app: &AppState, bot: &Bot) -> anyhow::Result<Read> {
    let codex = bot.runtime == BotRuntime::CodexCli;
    let mut out = Read::default();
    // The newest reply seen, to tell a rate limit the bot got past.
    let mut newest_reply: Option<DateTime<Utc>> = None;
    for path in files(app, bot) {
        let shown = path.display().to_string();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let len = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let cursor = app.db.usage_cursor(&bot.id, &shown)?;
        if cursor == len {
            continue;
        }
        let mut entries = Vec::new();
        let offset = crate::chat::transcript_lines::read_from(&path, cursor, |at, text| {
            let fallback = format!("{name}:{at}");
            match parse::line(text, codex, &fallback) {
                Some(Line::Reply {
                    key,
                    at,
                    model,
                    tokens,
                }) => {
                    newest_reply = newest_reply.max(Some(at));
                    entries.push(UsageEntry::Reply {
                        key,
                        day: day(at),
                        model,
                        tokens,
                    });
                }
                Some(Line::Prompt { key }) => entries.push(UsageEntry::Prompt { key }),
                Some(Line::TurnEnd { key }) => entries.push(UsageEntry::TurnEnd { key }),
                Some(Line::RateLimit { at, resets_at })
                    if out.limited.is_none_or(|(seen, _)| at > seen) =>
                {
                    out.limited = Some((at, resets_at));
                }
                Some(Line::RateLimit { .. }) | None => {}
            }
        })?;
        out.changed |= app
            .db
            .usage_apply(&bot.id, &bot.project_id, &shown, offset, &entries)?;
    }
    // A reply after the limit means the runtime got past it.
    if let (Some((at, _)), Some(reply)) = (out.limited, newest_reply) {
        if reply > at {
            out.limited = None;
        }
    }
    Ok(out)
}

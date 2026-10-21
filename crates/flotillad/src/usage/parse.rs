//! One transcript line, read for what it used: a reply's tokens, a prompt
//! or turn's end, or a rate limit.
//!
//! Claude Code writes each reply's `message.usage` on every line of that
//! reply (one per content block), under the same `message.id`; a key read
//! again only adds what grew. Codex's App Server reports its usage as
//! events, which the Codex worker writes into its observation record as
//! `usage` and `rate_limit` lines.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::db::Tokens;

/// What a line says about usage.
#[derive(Debug, Clone, PartialEq)]
pub enum Line {
    Reply {
        key: String,
        at: DateTime<Utc>,
        model: String,
        tokens: Tokens,
    },
    Prompt {
        key: String,
    },
    TurnEnd {
        key: String,
    },
    /// The runtime refused for its rate limit, until `resets_at` when it
    /// said.
    RateLimit {
        at: DateTime<Utc>,
        resets_at: Option<DateTime<Utc>>,
    },
}

fn num(v: &Value) -> u64 {
    v.as_u64().unwrap_or(0)
}

fn time(record: &Value) -> Option<DateTime<Utc>> {
    record
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&Utc))
}

/// The line, read as a Claude Code transcript's or (`codex`) as a Codex
/// observation record's. `fallback` keys a line with no id of its own.
pub fn line(text: &str, codex: bool, fallback: &str) -> Option<Line> {
    let record: Value = serde_json::from_str(text).ok()?;
    if codex {
        codex_line(&record, fallback)
    } else {
        claude_line(&record, fallback)
    }
}

fn claude_line(record: &Value, fallback: &str) -> Option<Line> {
    let at = time(record);
    match record.get("type").and_then(Value::as_str)? {
        "assistant" => {
            let message = record.get("message")?;
            if record.get("isApiErrorMessage").and_then(Value::as_bool) == Some(true) {
                let text = texts(&message["content"]);
                let limited = record.get("error").and_then(Value::as_str) == Some("rate_limit")
                    || is_limit_text(&text);
                let at = at?;
                return limited.then(|| Line::RateLimit {
                    at,
                    resets_at: super::reset::parse(&text, at),
                });
            }
            let usage = message.get("usage")?;
            let model = message.get("model").and_then(Value::as_str).unwrap_or("");
            if model == "<synthetic>" {
                return None;
            }
            let id = message.get("id").and_then(Value::as_str);
            Some(Line::Reply {
                key: format!("m:{}", id.unwrap_or(fallback)),
                at: at?,
                model: model.to_string(),
                tokens: Tokens {
                    input: num(&usage["input_tokens"]),
                    output: num(&usage["output_tokens"]),
                    cache_read: num(&usage["cache_read_input_tokens"]),
                    cache_write: num(&usage["cache_creation_input_tokens"]),
                },
            })
        }
        "user" if is_prompt(record) => {
            let id = record.get("uuid").and_then(Value::as_str);
            Some(Line::Prompt {
                key: format!("p:{}", id.unwrap_or(fallback)),
            })
        }
        _ => None,
    }
}

/// A user line that starts a turn: not a tool's result, nor one Claude
/// Code adds itself (meta lines, a compaction's summary).
fn is_prompt(record: &Value) -> bool {
    let flag = |name: &str| record.get(name).and_then(Value::as_bool) == Some(true);
    if flag("isMeta") || flag("isCompactSummary") || flag("isSidechain") {
        return false;
    }
    match &record["message"]["content"] {
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(blocks) => {
            !blocks.is_empty()
                && blocks
                    .iter()
                    .all(|b| b.get("type").and_then(Value::as_str) != Some("tool_result"))
        }
        _ => false,
    }
}

fn texts(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// Claude Code's words for its plan's limit: "Claude AI usage limit
/// reached|1760000000", "5-hour limit reached ∙ resets 3pm", "You've hit
/// your limit · resets 6pm (Europe/Rome)".
pub fn is_limit_text(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.contains("usage limit")
        || text.contains("limit reached")
        || text.contains("hit your limit")
        || text.contains("rate limit")
}

fn codex_line(record: &Value, fallback: &str) -> Option<Line> {
    let at = time(record);
    match record.get("type").and_then(Value::as_str)? {
        "usage" => {
            let usage = record.get("usage")?;
            let input = num(&usage["input_tokens"]);
            let cached = num(&usage["cached_input_tokens"]);
            let id = record.get("id").and_then(Value::as_str);
            Some(Line::Reply {
                key: format!("c:{}", id.unwrap_or(fallback)),
                at: at?,
                model: record["model"].as_str().unwrap_or("codex").to_string(),
                tokens: Tokens {
                    input: input.saturating_sub(cached),
                    output: num(&usage["output_tokens"]),
                    cache_read: cached,
                    cache_write: 0,
                },
            })
        }
        "system" if record.get("subtype").and_then(Value::as_str) == Some("turn_duration") => {
            Some(Line::TurnEnd {
                key: format!("e:{fallback}"),
            })
        }
        "rate_limit" => Some(Line::RateLimit {
            at: at?,
            resets_at: record
                .get("resets_at")
                .and_then(Value::as_str)
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|t| t.with_timezone(&Utc)),
        }),
        _ => None,
    }
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;

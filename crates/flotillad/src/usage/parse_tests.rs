use super::*;
use serde_json::json;

const AT: &str = "2026-10-21T10:00:00Z";

#[test]
fn a_claude_reply_counts_every_kind() {
    let text = json!({
        "type": "assistant", "timestamp": AT, "uuid": "u1",
        "message": { "id": "msg_1", "model": "claude-opus-5-5", "content": [],
            "usage": { "input_tokens": 10, "output_tokens": 20,
                       "cache_read_input_tokens": 300, "cache_creation_input_tokens": 40 } }
    })
    .to_string();
    let Some(Line::Reply {
        key, model, tokens, ..
    }) = line(&text, false, "f:0")
    else {
        panic!("not a reply");
    };
    assert_eq!(key, "m:msg_1");
    assert_eq!(model, "claude-opus-5-5");
    assert_eq!(
        tokens,
        Tokens {
            input: 10,
            output: 20,
            cache_read: 300,
            cache_write: 40
        }
    );
}

#[test]
fn prompts_start_turns_and_tool_results_dont() {
    let prompt = json!({"type": "user", "uuid": "u2", "message": {"content": "Do it"}});
    assert_eq!(
        line(&prompt.to_string(), false, "f:1"),
        Some(Line::Prompt { key: "p:u2".into() })
    );
    let result = json!({"type": "user", "uuid": "u3",
        "message": {"content": [{"type": "tool_result", "tool_use_id": "t", "content": "ok"}]}});
    assert_eq!(line(&result.to_string(), false, "f:2"), None);
    let meta = json!({"type": "user", "isMeta": true, "message": {"content": "x"}});
    assert_eq!(line(&meta.to_string(), false, "f:3"), None);
}

#[test]
fn a_synthetic_rate_limit_reply_is_a_rate_limit() {
    let text = json!({
        "type": "assistant", "timestamp": AT, "isApiErrorMessage": true, "error": "rate_limit",
        "message": { "id": "x", "model": "<synthetic>",
            "usage": { "input_tokens": 0, "output_tokens": 0 },
            "content": [{"type": "text", "text": "Claude AI usage limit reached|1761000000"}] }
    })
    .to_string();
    let Some(Line::RateLimit { resets_at, .. }) = line(&text, false, "f:4") else {
        panic!("not a rate limit");
    };
    assert_eq!(resets_at.map(|t| t.timestamp()), Some(1_761_000_000));
    let other = json!({
        "type": "assistant", "timestamp": AT, "isApiErrorMessage": true,
        "message": { "model": "<synthetic>", "content": [{"type": "text", "text": "API Error: 500"}] }
    });
    assert_eq!(line(&other.to_string(), false, "f:5"), None);
}

#[test]
fn codex_usage_splits_cached_input() {
    let text = json!({
        "type": "usage", "timestamp": AT, "id": "th:1200", "model": "gpt-5-codex",
        "usage": { "input_tokens": 1000, "cached_input_tokens": 800, "output_tokens": 200 }
    })
    .to_string();
    let Some(Line::Reply { key, tokens, .. }) = line(&text, true, "f:6") else {
        panic!("not a reply");
    };
    assert_eq!(key, "c:th:1200");
    assert_eq!(
        tokens,
        Tokens {
            input: 200,
            output: 200,
            cache_read: 800,
            cache_write: 0
        }
    );
    let end = json!({"type": "system", "subtype": "turn_duration", "timestamp": AT});
    assert_eq!(
        line(&end.to_string(), true, "f:7"),
        Some(Line::TurnEnd {
            key: "e:f:7".into()
        })
    );
    // Claude Code's own turn_duration isn't a turn: its prompts are.
    assert_eq!(line(&end.to_string(), false, "f:7"), None);
}

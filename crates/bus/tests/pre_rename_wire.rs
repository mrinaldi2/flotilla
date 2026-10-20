//! The proto packages were renamed from `hermes.*.v1` to `flotilla.*.v1`.
//! Binary frames carry field numbers, not names, so an app, phone or peer
//! from before the rename reads and writes them unchanged. (Encoders may
//! order fields differently, so frames are compared decoded.)
//! `fixtures/pre_rename_wire.json` holds every message fixture as the
//! pre-rename schema encoded it (`buf convert` against the old protos).

use std::path::PathBuf;

use bus::contract::wire::Envelope;
use prost::Message;
use serde_json::Value;

/// Message fixtures for fields added after the rename (0.20.0 to 0.22.0): the
/// old schema has no way to encode them, so they have no recorded frame.
const AFTER_THE_RENAME: &[&str] = &[
    "push.plan_change",
    "push.question_change",
    "request.ask_owner",
    "request.board_enable.columns",
    "request.board_enable.preset",
    "request.board_enable.release_gated",
    "request.item_assign.role",
    "request.item_create.task",
    "request.item_query.plan",
    "request.item_update.plan",
    "request.plan_create",
    "request.plan_create.goal",
    "request.plan_doc_read",
    "request.plan_drop",
    "request.plan_enable",
    "request.plan_get",
    "request.plan_node_get",
    "request.plan_propose",
    "request.plan_research_skip",
    "request.plan_rule",
    "request.plan_rule.send_back",
    "request.plan_submit",
    "request.plan_update",
    "request.question_answer",
    "request.question_dismiss",
    "request.question_list",
    "response.plan",
    "response.plan_doc",
    "response.plan_node",
    "response.planned.conflict",
    "response.planned.done",
    "response.planned.refused",
    "response.questioned.conflict",
    "response.questioned.done",
    "response.questioned.refused",
    "response.questions",
];

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn frames_from_before_the_rename_read_and_write_the_same() {
    let golden: Value = serde_json::from_str(
        &std::fs::read_to_string(fixtures().join("pre_rename_wire.json")).expect("golden"),
    )
    .expect("json");
    let envelopes = golden["envelopes"].as_object().expect("envelopes");
    let messages = fixtures().join("board/messages");
    let mut unrecorded: Vec<String> = std::fs::read_dir(&messages)
        .expect("messages")
        .map(|entry| {
            let path = entry.expect("entry").path();
            path.file_stem()
                .expect("stem")
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| !envelopes.contains_key(name))
        .collect();
    unrecorded.sort();
    assert_eq!(
        unrecorded, AFTER_THE_RENAME,
        "one recorded frame per fixture from before the rename"
    );
    for (name, hex_bytes) in envelopes {
        let old = hex::decode(hex_bytes.as_str().expect("hex")).expect("hex decodes");
        let text = std::fs::read_to_string(messages.join(format!("{name}.json"))).expect(name);
        let want: Envelope = serde_json::from_str(&text).expect(name);
        let read = Envelope::decode(old.as_slice())
            .unwrap_or_else(|e| panic!("{name}: an old frame does not decode: {e}"));
        assert_eq!(read, want, "{name}: an old frame reads differently");
        // What a renamed daemon sends, an old client reads the same way.
        let sent = Envelope::decode(want.encode_to_vec().as_slice()).expect("re-decode");
        assert_eq!(sent, read, "{name}");
    }
}

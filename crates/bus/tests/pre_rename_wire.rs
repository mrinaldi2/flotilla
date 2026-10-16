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
    let count = std::fs::read_dir(&messages).expect("messages").count();
    assert_eq!(envelopes.len(), count, "one recorded frame per fixture");
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

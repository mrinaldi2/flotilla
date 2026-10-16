//! A linked computer not yet on Flotilla (the Windows PC is upgraded after
//! the Mac): the peer link names no product, so a The Hermes 0.19 daemon's
//! handshake, sent exactly as that release builds it, links to a renamed
//! one, and the renamed one answers in the shape that release expects.

use crate::common;

use common::peers::{computer, project, wait_until};
use common::WsClient;
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn a_peer_from_before_the_rename_links() {
    let mac = computer(|_| {}).await;
    let mut client = WsClient::connect(&mac).await;
    project(&mut client, "phd").await;
    let invite = client
        .request(json!({
            "type": "create_peer_invite", "name": "win",
            "url": format!("ws://{}/peer", mac.addr)
        }))
        .await;
    let peer_id = invite["peer"]["id"].as_str().expect("peer id").to_string();
    let code = invite["invite"].as_str().expect("invite");
    // The code format is unchanged: a pre-rename `add_peer` parses it.
    let (url, token) = code.rsplit_once('#').expect("ws url # token");
    assert!(url.starts_with("ws://") && url.ends_with("/peer"), "{code}");

    // The Hermes 0.19's dialer (`peer/socket.rs` before the rename).
    let (mut socket, _) = tokio_tungstenite::connect_async(url).await.expect("dial");
    let hello = json!({
        "type": "peer_hello",
        "protocol": 1,
        "token": token,
        "daemon_id": "d-the-hermes-pc",
        "kanban": 2,
    });
    socket
        .send(Message::Text(hello.to_string()))
        .await
        .expect("hello");
    let reply = loop {
        match socket.next().await.expect("a reply").expect("frame") {
            Message::Text(text) => break serde_json::from_str::<Value>(&text).expect("json"),
            _ => continue,
        }
    };
    // What that release's dialer checks before it keeps the link.
    assert_eq!(reply["type"], "peer_hello_ok", "{reply}");
    assert_eq!(reply["protocol"], 1, "{reply}");
    assert_eq!(reply["kanban"], 2, "{reply}");
    assert!(reply["daemon_id"].as_str().is_some_and(|id| !id.is_empty()));
    wait_until("the old peer is online", || {
        mac.app.peers.is_online(&peer_id)
    })
    .await;
    assert_eq!(mac.app.peers.kanban(&peer_id), 2);
}

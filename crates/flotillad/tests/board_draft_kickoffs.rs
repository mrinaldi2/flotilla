//! One kickoff per discussion, even on a double click (0.19.0): the same
//! bot invited again to the same draft, unchanged, within the window, is
//! not sent the draft twice. An edit, an attachment, the window passing or
//! an uninvite in between each start the discussion afresh. The reply to
//! an invite is the same whether a kickoff went out or not.
//!
//! The kickoff is written to the bot's conversation before the invite is
//! answered, so the count read there after the reply is exact.

use std::time::Duration;

use crate::board_draft_discussion::{detail, draft, invite, shown, Draft, ENDED, KICKOFF};
use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use common::board::*;
use common::peer_files::{owner_attach, screenshot};
use common::tasks::project_with_bots;
use common::{TestDaemon, WsClient};

/// The kickoffs in the bot's conversation with the owner.
fn kickoffs(d: &TestDaemon, bot_id: &str) -> Vec<String> {
    let Some(conv) = d.app.db.dm_conversation(bot_id).unwrap() else {
        return Vec::new();
    };
    d.app
        .db
        .list_messages(&conv.id, None, 100)
        .unwrap()
        .into_iter()
        .filter(|m| m.body.contains(KICKOFF))
        .map(|m| m.body)
        .collect()
}

async fn setup() -> (common::tasks::Pair, WsClient, Draft) {
    let (pair, _bots) = project_with_bots(&["Lead", "Builder"]).await;
    let mut owner = WsClient::connect(&pair.d).await;
    let x = draft(&pair.d, &pair.ids, &mut owner).await;
    (pair, owner, x)
}

async fn retitle(d: &TestDaemon, owner: &mut WsClient, id: &str, title: &str) {
    let version = d.app.db.get_item(id).unwrap().unwrap().version;
    call(
        owner,
        Request::ItemUpdate(c::ItemUpdate {
            id: id.into(),
            expected_version: version,
            title: Some(title.into()),
            ..Default::default()
        }),
    )
    .await;
}

#[tokio::test]
async fn two_invites_in_quick_succession_send_one_kickoff() {
    let (pair, mut owner, x) = setup().await;
    let d = &pair.d;
    let first = detail(call(&mut owner, invite(&x.id, "Lead", false)).await);
    let second = detail(call(&mut owner, invite(&x.id, "Lead", false)).await);
    // Idempotent: the skipped kickoff changes nothing the owner is told.
    assert_eq!(first, second);
    assert_eq!(second.invited, std::slice::from_ref(&x.lead_id));
    assert_eq!(kickoffs(d, &x.lead_id).len(), 1);

    // Two devices at once (or a double click racing itself) send one too.
    let (mut a, mut b) = (WsClient::connect(d).await, WsClient::connect(d).await);
    let (ra, rb) = tokio::join!(
        call(&mut a, invite(&x.id, "Builder", false)),
        call(&mut b, invite(&x.id, "Builder", false)),
    );
    assert_eq!(detail(ra).invited, detail(rb).invited);
    assert_eq!(kickoffs(d, &x.builder_id).len(), 1);
    // The bot's session got it once.
    let term = shown(d, &x.builder_id, KICKOFF, 1).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        common::terminal(d, &x.builder_id).matches(KICKOFF).count(),
        1,
        "{term}"
    );
    // And each bot only its own.
    assert_eq!(kickoffs(d, &x.lead_id).len(), 1);
}

#[tokio::test]
async fn a_change_in_between_sends_the_draft_again() {
    let (pair, mut owner, x) = setup().await;
    let d = &pair.d;
    call(&mut owner, invite(&x.id, "Lead", false)).await;
    retitle(d, &mut owner, &x.id, "Unit conversion, metric").await;
    call(&mut owner, invite(&x.id, "Lead", false)).await;
    let sent = kickoffs(d, &x.lead_id);
    assert_eq!(sent.len(), 2, "{sent:?}");
    assert!(
        sent[1].contains("Title: Unit conversion, metric"),
        "{}",
        sent[1]
    );

    // A file added changes no version, but is a change all the same.
    owner_attach(&mut owner, &x.id, "second.png", &screenshot(4096)).await;
    call(&mut owner, invite(&x.id, "Lead", false)).await;
    let sent = kickoffs(d, &x.lead_id);
    assert_eq!(sent.len(), 3);
    assert!(sent[2].contains("second.png"), "{}", sent[2]);
    // Unchanged since, it is not sent again.
    call(&mut owner, invite(&x.id, "Lead", false)).await;
    assert_eq!(kickoffs(d, &x.lead_id).len(), 3);
}

#[tokio::test]
async fn after_the_window_the_draft_is_sent_again() {
    let (pair, mut owner, x) = setup().await;
    let d = &pair.d;
    d.app.kickoffs.window_for_tests(Duration::from_millis(400));
    call(&mut owner, invite(&x.id, "Lead", false)).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    call(&mut owner, invite(&x.id, "Lead", false)).await;
    assert_eq!(kickoffs(d, &x.lead_id).len(), 2);
    shown(d, &x.lead_id, KICKOFF, 2).await;
}

#[tokio::test]
async fn uninvited_then_invited_again_starts_afresh() {
    let (pair, mut owner, x) = setup().await;
    let d = &pair.d;
    call(&mut owner, invite(&x.id, "Lead", false)).await;
    let removed = detail(call(&mut owner, invite(&x.id, "Lead", true)).await);
    assert!(removed.invited.is_empty());
    shown(d, &x.lead_id, ENDED, 1).await;
    // Within the window, the draft unchanged: a new discussion all the same.
    let back = detail(call(&mut owner, invite(&x.id, "Lead", false)).await);
    assert_eq!(back.invited, std::slice::from_ref(&x.lead_id));
    assert_eq!(kickoffs(d, &x.lead_id).len(), 2);
    shown(d, &x.lead_id, KICKOFF, 2).await;
}

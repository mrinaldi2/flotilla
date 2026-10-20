use prost::Message;

use super::*;
use crate::kanban::feed::Change;

fn change<'a>(project_id: &'a str, item_id: &'a str) -> Change<'a> {
    Change {
        project_id,
        kind: ChangeKind::ItemUpserted,
        item_id,
        card: None,
        from_column: None,
    }
}

fn board_event(frame: &[u8]) -> c::BoardEvent {
    use bus::contract::wire::{envelope::Body, Envelope};
    match Envelope::decode(frame).expect("an envelope").body {
        Some(Body::BoardPush(c::BoardPush {
            push: Some(c::board_push::Push::BoardEvent(event)),
        })) => event,
        other => panic!("not a board event: {other:?}"),
    }
}

/// A connection that falls further behind than the feed keeps is told to
/// resync each project it watches, at the project's current seq, and then
/// gets the changes the feed still holds (ARCH-R8 F4).
#[tokio::test]
async fn a_lagging_connection_is_told_to_resync() {
    let feed = BoardFeed::with_backlog(2);
    let rx = feed.subscribe();
    let mut w = feed.writer();
    for item in ["H-1", "H-2", "H-3", "H-4", "H-5"] {
        w.publish(change("p", item));
    }
    drop(w);
    let projects = Arc::new(Mutex::new(HashSet::from(["p".to_string()])));
    let (out, mut frames) = mpsc::unbounded_channel();
    let task = tokio::spawn(forward(rx, feed.clone(), projects, out));

    let first = board_event(&frames.recv().await.expect("a frame"));
    assert_eq!(first.kind, c::BoardEventKind::Resync as i32);
    assert_eq!((first.project_id.as_str(), first.seq), ("p", 5));
    let kept: Vec<(String, u64)> = [frames.recv().await, frames.recv().await]
        .into_iter()
        .map(|f| board_event(&f.expect("a frame")))
        .map(|e| (e.item_id, e.seq))
        .collect();
    assert_eq!(kept, [("H-4".to_string(), 4), ("H-5".to_string(), 5)]);
    task.abort();
}

//! What a link sends, and how much file data it may hold unsent (0.19.0
//! K6). A peer's request is answered off the read loop and its answer
//! queued for the link's writer. Most answers are small, but each
//! `attachment_fetch` answer carries up to 512 KiB of a file. A peer that
//! asked for chunks and never read them would have this daemon queue them
//! all, so a link holds at most [`FILE_ANSWERS`] of those unsent. The next
//! one waits until the writer has sent one.

use std::sync::Arc;

use serde_json::Value;
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};

/// A frame for the link's writer, with the slot it holds until it is sent.
pub(super) struct Outgoing {
    pub frame: Value,
    _slot: Option<OwnedSemaphorePermit>,
}

impl From<Value> for Outgoing {
    fn from(frame: Value) -> Self {
        Self { frame, _slot: None }
    }
}

pub(super) type Out = mpsc::UnboundedSender<Outgoing>;

/// How many answers with file data one link may hold unsent. The linked
/// computer fetches one chunk at a time per file, so a few cover bots
/// reading several items at once.
pub(super) const FILE_ANSWERS: usize = 4;

/// The slots of one link.
pub(super) fn file_slots() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(FILE_ANSWERS))
}

fn carries_file_data(frame: &Value) -> bool {
    frame.get("type").and_then(Value::as_str) == Some("attachment_fetch")
}

/// Answers a request on the blocking pool and queues the answer. One that
/// carries file data first waits for a slot, and holds it until sent.
pub(super) fn answer_later(
    slots: &Arc<Semaphore>,
    out: Out,
    frame: Value,
    answer: impl FnOnce(&Value) -> Value + Send + 'static,
) {
    if !carries_file_data(&frame) {
        tokio::task::spawn_blocking(move || {
            let _ = out.send(answer(&frame).into());
        });
        return;
    }
    let slots = slots.clone();
    tokio::spawn(async move {
        let Ok(slot) = slots.acquire_owned().await else {
            return;
        };
        if let Ok(frame) = tokio::task::spawn_blocking(move || answer(&frame)).await {
            let _ = out.send(Outgoing {
                frame,
                _slot: Some(slot),
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;

    async fn next(rx: &mut mpsc::UnboundedReceiver<Outgoing>) -> Option<Outgoing> {
        tokio::time::timeout(Duration::from_millis(300), rx.recv())
            .await
            .ok()
            .flatten()
    }

    #[tokio::test]
    async fn a_link_holds_a_few_file_answers_unsent_never_all() {
        let (out, mut rx) = mpsc::unbounded_channel();
        let slots = file_slots();
        for n in 0..10 {
            let frame = json!({"type": "attachment_fetch", "req_id": n});
            answer_later(&slots, out.clone(), frame, |_| json!({"data": "512 KiB"}));
        }
        // The writer is stuck (the peer reads nothing): these stay queued.
        let mut unsent = Vec::new();
        while let Some(held) = next(&mut rx).await {
            unsent.push(held);
        }
        assert_eq!(unsent.len(), FILE_ANSWERS);
        // Other answers don't wait for a slot.
        answer_later(&slots, out.clone(), json!({"type": "ping"}), |_| json!({}));
        assert!(next(&mut rx).await.is_some_and(|o| o._slot.is_none()));
        // One sent frees a slot for the next.
        drop(unsent.pop());
        let fifth = next(&mut rx).await;
        assert!(fifth.is_some());
        assert!(next(&mut rx).await.is_none(), "still four unsent");
    }
}

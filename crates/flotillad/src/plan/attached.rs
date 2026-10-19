//! A gate's document before it is pushed (0.22.0): the copy the lead
//! attached to the node's document card. The owner reads and approves it
//! from that copy, so a gate never waits on the owner allowing a push; once
//! the document is on the default branch, the repository's copy wins.

use std::io::Read;

use crate::app::AppState;
use crate::kanban::attach::{artifacts_of, open_confined, root};
use crate::kanban::attachments::local_path;
use crate::kanban::model::{AttachmentKind, ItemAttachment, ItemType};

/// The largest attached document a gate reads.
const MAX_BYTES: u64 = 8 * 1024 * 1024;

/// Whether `name` is a document the owner reads at a gate.
fn is_document(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".md", ".markdown", ".html", ".htm", ".txt", ".pdf"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

/// The newest document attached to one of the node's document cards (its
/// design-type kickoff cards: "Write the PRD", "Write the design", …).
pub fn of_node(app: &AppState, project_id: &str, node_id: &str) -> Option<ItemAttachment> {
    app.db
        .board_read(|t| {
            let mut found = Vec::new();
            for card in t.plan_cards(project_id, node_id)? {
                if card.item_type == ItemType::Design {
                    found.extend(t.attachments(&card.id)?);
                }
            }
            Ok(found)
        })
        .ok()?
        .into_iter()
        .filter(|a| {
            !a.removed
                && matches!(a.kind, AttachmentKind::File | AttachmentKind::Doc)
                && !a.path.is_empty()
                && is_document(&a.name)
        })
        .max_by_key(|a| a.at)
}

/// The node whose gate, or one of whose documents, is at `path`.
pub fn node_of(app: &AppState, project_id: &str, path: &str) -> Option<String> {
    app.db
        .board_read(|t| t.plan_node_of_doc(project_id, path))
        .ok()
        .flatten()
}

/// The attached copy of the document at `path`, and its MIME type.
pub fn bytes(app: &AppState, project_id: &str, path: &str) -> Option<(Vec<u8>, String)> {
    let node = node_of(app, project_id, path)?;
    let att = of_node(app, project_id, &node)?;
    let artifacts = artifacts_of(app, project_id).ok()?;
    let file_path = local_path(&att, &artifacts)?;
    let file = open_confined(&file_path, &root(&artifacts)).ok()?;
    let mut data = Vec::new();
    file.take(MAX_BYTES).read_to_end(&mut data).ok()?;
    Some((data, att.mime))
}

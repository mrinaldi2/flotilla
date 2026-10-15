//! An item's files across a peer link, on the board's home (0.19.0 K6).
//! The bytes stay here. A linked computer fetches an attachment of a live
//! item of a project linked through it with `attachment_fetch`, a chunk at a
//! time, and caches it by its sha256. Its bot attaches a file of its own by
//! sending the bytes with `attachment_push` into an upload this daemon names
//! for that bot's stand-in, then `item_attach{upload_id}` through
//! `board_call`: the home never opens a path a peer names.

use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use bus::contract::board as c;
use bus::Peer;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::board_home::{home_link, stand_in};
use super::refuse;
use crate::actor::Actor;
use crate::app::AppState;
use crate::kanban::attach::artifacts_of;
use crate::kanban::attach::read_chunk;
use crate::kanban::attachments::{check_item_total, item_dir, CHUNK_MAX};
use crate::kanban::edit::Editor;
use crate::kanban::model::{AttachmentKind, ItemState};

/// The start of every upload id named for this stand-in's pushes: another
/// bot's upload, or the owner's, isn't one it can attach.
fn upload_prefix(stand_in_id: &str) -> String {
    let hash = hex::encode(Sha256::digest(stand_in_id.as_bytes()));
    format!("peer-{}-", &hash[..16])
}

/// Peers' chunks are written one at a time: what a stand-in has staged is
/// counted before each chunk lands, so two chunks at once can't both fit,
/// and an upload's chunks can't overlap.
static PUSHES: Mutex<()> = Mutex::new(());

/// An upload that no chunk has reached for this long was abandoned (its
/// link dropped mid-file): swept when its bot starts another.
const ABANDONED: Duration = Duration::from_secs(10 * 60);

/// The bytes this stand-in has staged in an item's uploads, sweeping the
/// abandoned ones first when `sweep`. Never swept otherwise: a staged
/// upload is only removed by the attach that takes it.
fn staged_by(uploads: &Path, prefix: &str, sweep: bool) -> u64 {
    let ours = format!("up-{prefix}");
    let mut total = 0;
    for entry in fs::read_dir(uploads).into_iter().flatten().flatten() {
        if !entry.file_name().to_string_lossy().starts_with(&ours) {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let idle = meta.modified().ok().and_then(|at| at.elapsed().ok());
        if sweep && idle.is_some_and(|idle| idle > ABANDONED) {
            fs::remove_file(entry.path()).ok();
            continue;
        }
        total += meta.len();
    }
    total
}

fn text<'a>(frame: &'a Value, key: &str) -> &'a str {
    frame.get(key).and_then(Value::as_str).unwrap_or_default()
}

/// A forwarded `item_attach`: never a path, and an upload only of this
/// stand-in's own pushes.
pub(super) fn check_attach(stand_in: &bus::Bot, args: &Value) -> anyhow::Result<()> {
    let given = |key: &str| !text(args, key).trim().is_empty();
    if given("file") {
        return Err(refuse(
            "invalid_request",
            "the board's home can't read a file on your computer; update Flotilla there, \
             which sends the file itself",
        ));
    }
    if given("upload_id") && !text(args, "upload_id").starts_with(&upload_prefix(&stand_in.id)) {
        return Err(refuse(
            "forbidden",
            "that upload isn't one your computer sent",
        ));
    }
    Ok(())
}

/// `attachment_push {project_id, bot_id, item_id, upload_id?, name, offset,
/// total, data}`: one chunk of a file the peer's bot attaches, at most
/// 512 KiB, base64. The first (offset 0, no upload id) starts an upload and
/// its answer names it; the caps and the type are checked as for the
/// owner's uploads.
pub(super) fn serve_push(app: &Arc<AppState>, peer: &Peer, frame: &Value) -> anyhow::Result<Value> {
    let link = home_link(app, peer, frame)?;
    let bot = stand_in(app, peer, &link, text(frame, "bot_id"))?;
    let offset = frame.get("offset").and_then(Value::as_u64).unwrap_or(0);
    let prefix = upload_prefix(&bot.id);
    let upload_id = match text(frame, "upload_id") {
        "" if offset == 0 => {
            let nonce: String = bus::new_id()
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .take(24)
                .collect();
            format!("{prefix}{nonce}")
        }
        "" => anyhow::bail!("name the upload this chunk continues"),
        id if id.starts_with(&prefix) => id.to_string(),
        _ => {
            return Err(refuse(
                "forbidden",
                "that upload isn't one your computer sent",
            ))
        }
    };
    let data = base64::engine::general_purpose::STANDARD
        .decode(text(frame, "data"))
        .map_err(|_| anyhow::anyhow!("a chunk's data is base64"))?;
    let item_id = text(frame, "item_id");
    let ed = Editor {
        app,
        project_id: &link.project_id,
        actor: Actor::Bot {
            id: &bot.id,
            project_id: &link.project_id,
        },
    };
    // Before anything about the item is told: a draft the stand-in isn't
    // invited to doesn't exist for it.
    ed.own_item(item_id)?;
    let _one_at_a_time = PUSHES.lock().unwrap_or_else(|e| e.into_inner());
    // Staged bytes count toward the item's cap with what it stores: a peer
    // that starts uploads and never attaches them can't fill this disk.
    let uploads = item_dir(&artifacts_of(app, &link.project_id)?, item_id).join(".uploads");
    let staged = staged_by(&uploads, &prefix, offset == 0);
    let stored = app.db.board_read(|t| t.stored_bytes(item_id, ""))?;
    check_item_total(&app.cfg.attachments, data.len() as u64, stored + staged)?;
    let received = ed.upload(&c::AttachmentUpload {
        item_id: item_id.to_string(),
        upload_id: upload_id.clone(),
        offset,
        data,
        total: frame.get("total").and_then(Value::as_u64).unwrap_or(0),
        name: text(frame, "name").to_string(),
    })?;
    Ok(json!({ "upload_id": upload_id, "received": received }))
}

/// `attachment_fetch {project_id, attachment_id, offset}`: a chunk of an
/// attachment's bytes, at most 512 KiB, base64, with the sha256 they must
/// hash to. Only an image or a file attached to a live item of the linked
/// project: a draft's, a discarded item's, a detached one and another
/// project's don't exist here for a peer, and a doc has no bytes of its own.
pub(super) fn serve_fetch(
    app: &Arc<AppState>,
    peer: &Peer,
    frame: &Value,
) -> anyhow::Result<Value> {
    let link = home_link(app, peer, frame)?;
    let id = text(frame, "attachment_id");
    let att = app
        .db
        .board_read(|t| {
            let Some(att) = t.attachment(id)? else {
                return Ok(None);
            };
            let ours = t.item_project(&att.item_id)?.as_deref() == Some(link.project_id.as_str());
            let live = t.item_state(&att.item_id)? == Some(ItemState::Live);
            // A detached file's bytes stay for the home's history, not to
            // go on being handed out.
            Ok((ours && live && !att.removed).then_some(att))
        })?
        .ok_or_else(|| refuse("not_found", format!("no attachment {id}")))?;
    if att.kind == AttachmentKind::Doc {
        return Err(refuse(
            "invalid_request",
            "a doc names an artifact or a URL; it has no bytes of its own to fetch",
        ));
    }
    let chunk = read_chunk(
        app,
        &c::AttachmentRead {
            attachment_id: att.id.clone(),
            offset: frame.get("offset").and_then(Value::as_u64).unwrap_or(0),
            limit: CHUNK_MAX as u32,
        },
    )?;
    Ok(json!({
        "attachment_id": att.id,
        "sha256": att.sha256,
        "offset": chunk.offset,
        "total": chunk.total,
        "data": base64::engine::general_purpose::STANDARD.encode(&chunk.data),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_upload_id_names_its_stand_in() {
        let prefix = upload_prefix("bot-1");
        assert!(
            prefix.starts_with("peer-") && prefix.len() == 22,
            "{prefix}"
        );
        assert_ne!(prefix, upload_prefix("bot-2"));
        // With a 24-character nonce, still an upload id the owner's uploads
        // take: at most 64 letters, digits, '-' or '_'.
        assert!(prefix.len() + 24 <= 64);
    }
}

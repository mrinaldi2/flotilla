//! An item's files on a board homed on the Mac and read from a linked PC
//! (0.19.0 K6): what the file tests share.

use std::path::PathBuf;

use bus::contract::board::{self as c, board_request::Request, board_response::Response};
use hermesd::kanban::attachments::local_path;

use super::board::{call, response};
use super::{TestDaemon, WsClient};

/// A PNG of `len` bytes: its signature, then filler.
pub fn screenshot(len: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.resize(len, 7);
    bytes
}

/// The owner attaches `bytes` as `name` to an item, uploading them in
/// chunks; returns the attachment.
pub async fn owner_attach(
    owner: &mut WsClient,
    item: &str,
    name: &str,
    bytes: &[u8],
) -> c::ItemAttachment {
    let upload_id = format!("up-{}", bytes.len());
    let mut offset = 0usize;
    for chunk in bytes.chunks(512 * 1024) {
        let reply = call(
            owner,
            Request::AttachmentUpload(c::AttachmentUpload {
                item_id: item.into(),
                upload_id: upload_id.clone(),
                offset: offset as u64,
                data: chunk.to_vec(),
                total: bytes.len() as u64,
                name: name.into(),
            }),
        )
        .await;
        offset += chunk.len();
        assert!(matches!(response(reply), Response::Upload(_)));
    }
    let attached = call(
        owner,
        Request::ItemAttach(c::ItemAttach {
            id: item.into(),
            upload_id: Some(upload_id),
            name: Some(name.into()),
            ..Default::default()
        }),
    )
    .await;
    match response(attached) {
        Response::Item(detail) => detail
            .attachments
            .into_iter()
            .rev()
            .find(|a| a.name == name)
            .expect("attached"),
        other => panic!("expected the item, got {other:?}"),
    }
}

/// The owner links a URL to an item as a doc; returns its id.
pub async fn owner_link_doc(owner: &mut WsClient, item: &str, url: &str) -> String {
    let attached = call(
        owner,
        Request::ItemAttach(c::ItemAttach {
            id: item.into(),
            url: Some(url.into()),
            ..Default::default()
        }),
    )
    .await;
    match response(attached) {
        Response::Item(detail) => {
            detail
                .attachments
                .into_iter()
                .find(|a| a.url == url)
                .expect("linked")
                .id
        }
        other => panic!("expected the item, got {other:?}"),
    }
}

/// Where the home keeps an attachment's bytes.
pub fn stored(home: &TestDaemon, project_id: &str, attachment_id: &str) -> PathBuf {
    let db = &home.app.db;
    let att = db
        .board_read(|t| t.attachment(attachment_id))
        .unwrap()
        .expect("attachment");
    let project = db.get_project(project_id).unwrap().expect("project");
    let artifacts = hermesd::paths::artifacts_dir(&home.app.cfg, &project.dir_name);
    local_path(&att, &artifacts).expect("bytes")
}

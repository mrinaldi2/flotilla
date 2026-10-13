//! Attaching (0.19.0 K4b): files are copied into the project's artifacts
//! under `items/<item-id>/`, named by the first bytes of their sha256 and
//! their own safe name, so the same bytes are stored once per name and a
//! linked computer can cache them by hash. A doc links an artifact or a URL
//! and stores nothing.
//!
//! The owner's apps upload in chunks (`attachment_upload`) into
//! `items/<item-id>/.uploads/<upload-id>`, then attach the upload. Nothing
//! about an upload is kept in memory, so any connection may finish it.

use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use bus::contract::board as c;

use super::attachments::{
    check_content, check_item_total, check_size, file_type, item_dir, local_path, CHUNK_MAX,
};
use super::edit::Editor;
use super::feed::ChangeKind;
use super::model::{AttachmentKind, ItemAttachment};
use crate::app::AppState;
use crate::db::NewAttachment;
use crate::decisions::{invalid, not_found};

mod store;

use store::{doc_artifact, doc_url, open_confined, root, Stored};

/// Where an attachment's content comes from.
pub enum Source {
    /// A finished upload of the owner's apps.
    Upload(String),
    /// A file of a bot's, in its own folder or the project's artifacts
    /// (`roots`, canonical): checked again once it is open.
    File { path: PathBuf, roots: Vec<PathBuf> },
    /// A project artifact, linked as a doc.
    Artifact(String),
    /// An external document, linked as a doc.
    Url(String),
}

/// The project's artifacts folder.
pub fn artifacts_of(app: &AppState, project_id: &str) -> anyhow::Result<PathBuf> {
    let project = app
        .db
        .get_project(project_id)?
        .ok_or_else(|| not_found(format!("no project {project_id}")))?;
    Ok(crate::paths::artifacts_dir(&app.cfg, &project.dir_name))
}

fn upload_path(artifacts: &Path, item_id: &str, upload_id: &str) -> anyhow::Result<PathBuf> {
    let valid = !upload_id.is_empty()
        && upload_id.len() <= 64
        && upload_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    anyhow::ensure!(
        valid,
        invalid("an upload id is 1 to 64 letters, digits, '-' or '_'")
    );
    // Prefixed, so no id is a name Windows keeps for a device (`CON`, `NUL`).
    Ok(item_dir(artifacts, item_id)
        .join(".uploads")
        .join(format!("up-{upload_id}")))
}

impl Editor<'_> {
    /// Take one chunk of an upload; returns the bytes received so far. The
    /// first chunk checks the file's type and size against the caps.
    pub fn upload(&self, req: &c::AttachmentUpload) -> anyhow::Result<u64> {
        self.own_item(&req.item_id)?;
        self.refuse_discarded(&req.item_id)?;
        anyhow::ensure!(
            req.data.len() <= CHUNK_MAX,
            invalid(format!("a chunk carries at most {CHUNK_MAX} bytes"))
        );
        let artifacts = artifacts_of(self.app, self.project_id)?;
        let path = upload_path(&artifacts, &req.item_id, &req.upload_id)?;
        let limits = &self.app.cfg.attachments;
        let ft = file_type(limits, &req.name)?;
        anyhow::ensure!(
            req.total > 0,
            invalid("an upload names its file's total size")
        );
        let on_item = self.app.db.board_read(|t| t.stored_bytes(&req.item_id, ""))?;
        check_size(limits, ft, req.total, on_item)?;
        if req.offset == 0 {
            check_content(ft, &req.data[..req.data.len().min(16)])?;
            fs::create_dir_all(path.parent().expect("an upload has a folder"))?;
            // A fresh file, never one a link put there to be written through.
            fs::remove_file(&path).ok();
            OpenOptions::new().write(true).create_new(true).open(&path)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|_| invalid("no such upload: send its first chunk with offset 0"))?;
        let len = file.metadata()?.len();
        anyhow::ensure!(
            len == req.offset,
            invalid(format!(
                "the upload has {len} bytes; send the chunk at that offset"
            ))
        );
        let received = len + req.data.len() as u64;
        anyhow::ensure!(
            received <= req.total,
            invalid("the chunks are longer than the file's total")
        );
        file.seek(SeekFrom::End(0))?;
        file.write_all(&req.data)?;
        Ok(received)
    }

    /// Attach a file, an upload, an artifact or a URL to the item.
    pub fn attach(&self, req: &c::ItemAttach, source: Source) -> anyhow::Result<ItemAttachment> {
        self.own_item(&req.id)?;
        self.refuse_discarded(&req.id)?;
        let artifacts = artifacts_of(self.app, self.project_id)?;
        let caption = req.caption.as_deref().unwrap_or_default().trim();
        let named = req.name.as_deref().map(str::trim).filter(|n| !n.is_empty());
        let stored = match source {
            Source::Upload(upload_id) => {
                let staged = upload_path(&artifacts, &req.id, &upload_id)?;
                anyhow::ensure!(staged.is_file(), invalid(format!("no upload {upload_id}")));
                let name = named.ok_or_else(|| invalid("name the uploaded file"))?;
                let uploads = root(staged.parent().expect("an upload has a folder"));
                let file = open_confined(&staged, &uploads)?;
                let stored = self.store(&artifacts, &req.id, file, name)?;
                fs::remove_file(&staged).ok();
                stored
            }
            Source::File { path, roots } => {
                let own = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                let file = open_confined(&path, &roots)?;
                self.store(&artifacts, &req.id, file, named.unwrap_or(own))?
            }
            Source::Artifact(rel) => doc_artifact(&artifacts, &rel, named)?,
            Source::Url(url) => doc_url(&url, named)?,
        };
        let limits = &self.app.cfg.attachments;
        self.published(ChangeKind::ItemUpserted, None, || {
            let att = self.app.db.board_tx(|t| {
                // Checked again where it is written: two attaches at once
                // can't both fit the item's cap.
                if stored.kind != AttachmentKind::Doc {
                    let on_item = t.stored_bytes(&req.id, &stored.path)?;
                    check_item_total(limits, stored.bytes, on_item)?;
                }
                t.add_attachment(
                    &NewAttachment {
                        item_id: &req.id,
                        kind: stored.kind,
                        name: &stored.name,
                        mime: stored.mime,
                        bytes: stored.bytes,
                        sha256: &stored.sha256,
                        path: &stored.path,
                        url: &stored.url,
                        caption,
                    },
                    &self.actor,
                )
            })?;
            Ok((att, Some(req.id.clone())))
        })
    }

    /// Detach one. Its bytes stay, so the history can still show it.
    pub fn detach(&self, req: &c::ItemDetach) -> anyhow::Result<bool> {
        self.own_item(&req.id)?;
        self.refuse_discarded(&req.id)?;
        self.published(ChangeKind::ItemUpserted, None, || {
            let removed = self
                .app
                .db
                .board_tx(|t| t.detach(&req.id, &req.attachment_id, &self.actor))?;
            Ok((removed, removed.then(|| req.id.clone())))
        })
    }

    /// Copy `from` in under its type's checks and the caps.
    fn store(
        &self,
        artifacts: &Path,
        item_id: &str,
        from: fs::File,
        name: &str,
    ) -> anyhow::Result<Stored> {
        let db = &self.app.db;
        let on_item = |path: &str| db.board_read(|t| t.stored_bytes(item_id, path));
        store::store(
            &self.app.cfg.attachments,
            artifacts,
            item_id,
            from,
            name,
            &on_item,
        )
    }
}

/// An item's attachment bytes, a chunk at a time.
pub fn read_chunk(app: &AppState, req: &c::AttachmentRead) -> anyhow::Result<c::AttachmentChunk> {
    let (att, project_id) = app
        .db
        .board_read(|t| {
            let Some(att) = t.attachment(&req.attachment_id)? else {
                return Ok(None);
            };
            Ok(t.item_project(&att.item_id)?.map(|p| (att, p)))
        })?
        .ok_or_else(|| not_found(format!("no attachment {}", req.attachment_id)))?;
    let artifacts = artifacts_of(app, &project_id)?;
    let path =
        local_path(&att, &artifacts).ok_or_else(|| invalid("a URL has no bytes here; open it"))?;
    let mut file = open_confined(&path, &root(&artifacts))?;
    let total = file.metadata()?.len();
    let limit = match req.limit {
        0 => CHUNK_MAX,
        n => (n as usize).min(CHUNK_MAX),
    };
    file.seek(SeekFrom::Start(req.offset.min(total)))?;
    let mut data = Vec::with_capacity(limit);
    file.take(limit as u64).read_to_end(&mut data)?;
    Ok(c::AttachmentChunk {
        attachment_id: att.id,
        offset: req.offset,
        data,
        total,
        mime: att.mime,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kanban::attachments::safe_name;

    #[test]
    fn an_upload_id_is_never_a_windows_device_name() {
        for id in ["CON", "nul", "COM1", "LPT9", "aux"] {
            let path = upload_path(Path::new("/a"), "H-1", id).expect(id);
            let name = path.file_name().unwrap().to_str().unwrap();
            assert_eq!(safe_name(name), name, "{id}");
            assert!(path.ends_with(Path::new("items/H-1/.uploads").join(name)));
        }
    }
}

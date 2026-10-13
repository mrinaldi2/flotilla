//! Attaching (0.19.0 K4b): files are copied into the project's artifacts
//! under `items/<item-id>/`, named by the first bytes of their sha256 and
//! their own safe name, so the same bytes are stored once per name and a
//! linked computer can cache them by hash. A doc links an artifact or a URL
//! and stores nothing.
//!
//! The owner's apps upload in chunks (`attachment_upload`) into
//! `items/<item-id>/.uploads/<upload-id>`, then attach the upload. Nothing
//! about an upload is kept in memory, so any connection may finish it.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use bus::contract::board as c;
use sha2::{Digest, Sha256};

use super::attachments::{
    check_content, check_size, file_type, item_dir, local_path, safe_name, CHUNK_MAX,
};
use super::edit::Editor;
use super::feed::ChangeKind;
use super::model::{AttachmentKind, ItemAttachment};
use crate::app::AppState;
use crate::db::NewAttachment;
use crate::decisions::{invalid, not_found};

/// Where an attachment's content comes from.
pub enum Source {
    /// A finished upload of the owner's apps.
    Upload(String),
    /// A file of a bot's, already checked to be its own or the project's.
    File(PathBuf),
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
    Ok(item_dir(artifacts, item_id)
        .join(".uploads")
        .join(upload_id))
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
        let on_item = self.app.db.board_read(|t| t.attached_bytes(&req.item_id))?;
        check_size(limits, ft, req.total, on_item)?;
        if req.offset == 0 {
            check_content(ft, &req.data[..req.data.len().min(16)])?;
            fs::create_dir_all(path.parent().expect("an upload has a folder"))?;
            File::create(&path)?;
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
                let stored = self.store(&artifacts, &req.id, &staged, name)?;
                fs::remove_file(&staged).ok();
                stored
            }
            Source::File(path) => {
                let own = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();
                self.store(&artifacts, &req.id, &path, named.unwrap_or(own))?
            }
            Source::Artifact(rel) => doc_artifact(&artifacts, &rel, named)?,
            Source::Url(url) => doc_url(&url, named)?,
        };
        self.published(ChangeKind::ItemUpserted, None, || {
            let att = self.app.db.board_tx(|t| {
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
        from: &Path,
        name: &str,
    ) -> anyhow::Result<Stored> {
        let limits = &self.app.cfg.attachments;
        let ft = file_type(limits, name)?;
        let bytes = fs::metadata(from)?.len();
        let on_item = self.app.db.board_read(|t| t.attached_bytes(item_id))?;
        check_size(limits, ft, bytes, on_item)?;
        let mut head = [0u8; 16];
        let read = File::open(from)?.read(&mut head)?;
        check_content(ft, &head[..read])?;
        let sha256 = hash(from)?;
        let name = safe_name(name);
        let file_name = format!("{}-{name}", &sha256[..12]);
        let dir = item_dir(artifacts, item_id);
        fs::create_dir_all(&dir)?;
        let to = dir.join(&file_name);
        if !to.is_file() {
            let partial = dir.join(format!(".{file_name}.part"));
            fs::copy(from, &partial)?;
            fs::rename(&partial, &to)?;
        }
        let rel = to
            .strip_prefix(artifacts)
            .map_err(|_| anyhow::anyhow!("an item's folder is in the artifacts"))?;
        Ok(Stored {
            kind: ft.kind,
            name,
            mime: ft.mime,
            bytes,
            sha256,
            path: slash_path(rel),
            url: String::new(),
        })
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
    let mut file = File::open(&path)?;
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

struct Stored {
    kind: AttachmentKind,
    name: String,
    mime: &'static str,
    bytes: u64,
    sha256: String,
    path: String,
    url: String,
}

fn hash(path: &Path) -> anyhow::Result<String> {
    let mut file = File::open(path)?;
    let mut sha = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        sha.update(&buf[..n]);
    }
    Ok(hex::encode(sha.finalize()))
}

/// A relative path with `/` between its parts, on every platform.
fn slash_path(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// A project artifact, by its path in the artifacts folder.
fn doc_artifact(artifacts: &Path, rel: &str, named: Option<&str>) -> anyhow::Result<Stored> {
    let parts: Vec<&str> = rel.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    anyhow::ensure!(
        !parts.is_empty() && !parts.iter().any(|p| *p == ".." || *p == "."),
        invalid("name an artifact by its path inside the artifacts folder")
    );
    let path = parts
        .iter()
        .fold(artifacts.to_path_buf(), |dir, p| dir.join(p));
    anyhow::ensure!(path.is_file(), invalid(format!("no artifact {rel}")));
    let name = named.unwrap_or(parts[parts.len() - 1]);
    Ok(Stored {
        kind: AttachmentKind::Doc,
        name: name.to_string(),
        mime: "",
        bytes: fs::metadata(&path)?.len(),
        sha256: String::new(),
        path: parts.join("/"),
        url: String::new(),
    })
}

fn doc_url(url: &str, named: Option<&str>) -> anyhow::Result<Stored> {
    let url = url.trim();
    anyhow::ensure!(
        url.starts_with("https://") || url.starts_with("http://"),
        invalid("a linked document's URL starts with https:// or http://")
    );
    Ok(Stored {
        kind: AttachmentKind::Doc,
        name: named.unwrap_or(url).to_string(),
        mime: "",
        bytes: 0,
        sha256: String::new(),
        path: String::new(),
        url: url.to_string(),
    })
}

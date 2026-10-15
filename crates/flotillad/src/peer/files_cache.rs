//! An item's files on a computer linked to the board's home (0.19.0 K6).
//! The bytes stay on the home. A bot here reads an attachment from a local
//! copy, fetched on first read with `attachment_fetch` into the project's
//! artifacts under `.cache/attachments/<sha256>.<ext>`, so the bots here may
//! read it as they read any artifact. A copy is used only once its bytes
//! hash to the record's sha256, checked again on every use, and is written
//! to a fresh `.part` file renamed into place, so a half-written or swapped
//! file is never handed out. A bot here attaches a file of its own by
//! sending its bytes (`attachment_push`), which leaves a copy in the cache.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use base64::Engine;
use bus::ProjectLink;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::app::AppState;
use crate::kanban::attach::{artifacts_of, open_confined};
use crate::kanban::attachments::{check_content, check_file_size, file_type, FileType, CHUNK_MAX};

/// An attachment as the home's `item_get` lists it.
pub(crate) struct Wanted<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub sha256: &'a str,
    pub bytes: u64,
}

fn b64() -> &'static base64::engine::GeneralPurpose {
    &base64::engine::general_purpose::STANDARD
}

/// A project's cache folder.
struct Cache {
    /// As the bots here are told it, like the artifacts' other paths (on
    /// Windows, without the `\\?\` prefix a canonical path has).
    shown: PathBuf,
    /// Canonical: inside the artifacts even once links are resolved. Copies
    /// are written here, and a shown path must still resolve inside it.
    real: PathBuf,
}

fn cache_dir(app: &AppState, project_id: &str) -> anyhow::Result<Cache> {
    let artifacts = artifacts_of(app, project_id)?;
    let shown = artifacts.join(".cache").join("attachments");
    fs::create_dir_all(&shown)?;
    let real = shown.canonicalize()?;
    anyhow::ensure!(
        real.starts_with(artifacts.canonicalize()?),
        "the attachment cache isn't inside the project's artifacts"
    );
    Ok(Cache { shown, real })
}

/// A sha256 as the home records it: 64 lower-case hex digits, which also
/// makes it a file name on every platform.
fn valid_sha(sha256: &str) -> bool {
    sha256.len() == 64
        && sha256
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn hash_of(mut file: File) -> anyhow::Result<String> {
    let mut sha = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            return Ok(hex::encode(sha.finalize()));
        }
        sha.update(&buf[..n]);
    }
}

/// The cached copy, when it is there and its bytes still hash to `sha256`.
/// One that doesn't, or is a link out of the cache, is removed.
fn hit(dir: &Path, path: &Path, sha256: &str) -> Option<PathBuf> {
    fs::symlink_metadata(path).ok()?;
    let held = open_confined(path, &[dir.to_path_buf()]).and_then(hash_of);
    if held.is_ok_and(|held| held == sha256) {
        return Some(path.to_path_buf());
    }
    tracing::warn!(path = %path.display(), "cached attachment doesn't match its sha256; refetching");
    fs::remove_file(path).ok();
    None
}

/// A fresh `.part` file in the cache, never one a link put there.
fn part_file(dir: &Path, sha256: &str) -> anyhow::Result<(PathBuf, File)> {
    let nonce: String = bus::new_id()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(12)
        .collect();
    let part = dir.join(format!(".{}-{nonce}.part", &sha256[..16]));
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&part)?;
    Ok((part, file))
}

/// Moves a checked copy to its name; the same bytes may be there already.
fn put_in_place(part: &Path, to: &Path) {
    if fs::rename(part, to).is_err() {
        fs::remove_file(part).ok();
    }
}

/// The local copy of a home's attachment, fetched on first read under this
/// computer's own caps and type allowlist.
pub(crate) async fn local_copy(
    app: &AppState,
    link: &ProjectLink,
    want: &Wanted<'_>,
) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(valid_sha(want.sha256), "its record has no valid sha256");
    let ft = file_type(&app.cfg.attachments, want.name)?;
    check_file_size(&app.cfg.attachments, ft.kind, want.bytes)?;
    let cache = cache_dir(app, &link.project_id)?;
    let name = format!("{}.{}", want.sha256, ft.ext);
    let target = cache.shown.join(&name);
    if let Some(path) = hit(&cache.real, &target, want.sha256) {
        return Ok(path);
    }
    let (part, mut out) = part_file(&cache.real, want.sha256)?;
    let fetched = fetch(app, link, want, &mut out).await;
    drop(out);
    match fetched {
        Ok(()) => put_in_place(&part, &cache.real.join(&name)),
        Err(e) => {
            fs::remove_file(&part).ok();
            return Err(e);
        }
    }
    hit(&cache.real, &target, want.sha256)
        .ok_or_else(|| anyhow::anyhow!("its copy didn't keep its sha256; try again"))
}

/// Fetches the bytes into `out`, refusing any that aren't the record's.
async fn fetch(
    app: &AppState,
    link: &ProjectLink,
    want: &Wanted<'_>,
    out: &mut File,
) -> anyhow::Result<()> {
    let mut sha = Sha256::new();
    let mut got = 0u64;
    loop {
        let frame = json!({
            "type": "attachment_fetch", "project_id": link.project_id,
            "attachment_id": want.id, "offset": got,
        });
        let reply = app.peers.request(&link.peer_id, frame).await?;
        anyhow::ensure!(
            reply["sha256"].as_str() == Some(want.sha256)
                && reply["total"].as_u64() == Some(want.bytes)
                && reply["offset"].as_u64() == Some(got),
            "the board's home answered for other bytes than its record names"
        );
        let data = b64()
            .decode(reply["data"].as_str().unwrap_or_default())
            .map_err(|_| anyhow::anyhow!("unreadable bytes from the board's home"))?;
        anyhow::ensure!(
            got + data.len() as u64 <= want.bytes,
            "the board's home sent more bytes than the attachment has"
        );
        if data.is_empty() {
            anyhow::ensure!(got == want.bytes, "the board's home sent too few bytes");
            break;
        }
        sha.update(&data);
        out.write_all(&data)?;
        got += data.len() as u64;
        if got == want.bytes {
            break;
        }
    }
    anyhow::ensure!(
        hex::encode(sha.finalize()) == want.sha256,
        "sha256 mismatch: the bytes from the board's home don't hash to its record; nothing \
         was kept"
    );
    out.sync_all()?;
    Ok(())
}

/// A file of a bot's here, to attach on the home.
pub(crate) struct Push<'a> {
    pub link: &'a ProjectLink,
    pub bot_id: &'a str,
    pub item_id: &'a str,
    /// Canonical, inside the folders it may come from (`roots`).
    pub path: &'a Path,
    pub roots: &'a [PathBuf],
    pub name: &'a str,
}

/// Sends the file to the board's home as an upload for the item, opened
/// once and checked as an attach checks it; returns the upload's id there.
/// Its bytes are then in the cache under their sha256.
pub(crate) async fn push(app: &AppState, push: &Push<'_>) -> anyhow::Result<String> {
    let limits = &app.cfg.attachments;
    let ft = file_type(limits, push.name)?;
    let mut file = open_confined(push.path, push.roots)?;
    let total = file.metadata()?.len();
    check_file_size(limits, ft.kind, total)?;
    let mut head = Vec::with_capacity(16);
    (&mut file).take(16).read_to_end(&mut head)?;
    check_content(ft, &head)?;
    file.seek(SeekFrom::Start(0))?;
    let dir = cache_dir(app, &push.link.project_id)?.real;
    let (part, mut copy) = part_file(&dir, &"0".repeat(16))?;
    let sent = send(app, push, ft, &mut file, total, &mut copy).await;
    drop(copy);
    match sent {
        Ok((upload_id, sha256)) => {
            put_in_place(&part, &dir.join(format!("{sha256}.{}", ft.ext)));
            Ok(upload_id)
        }
        Err(e) => {
            fs::remove_file(&part).ok();
            Err(e)
        }
    }
}

async fn send(
    app: &AppState,
    push: &Push<'_>,
    ft: FileType,
    file: &mut File,
    total: u64,
    copy: &mut File,
) -> anyhow::Result<(String, String)> {
    let mut sha = Sha256::new();
    let mut upload_id = Value::Null;
    let mut sent = 0u64;
    let mut buf = vec![0u8; CHUNK_MAX];
    let mut reader = file.take(total);
    while sent < total || sent == 0 {
        let n = reader.read(&mut buf)?;
        anyhow::ensure!(
            n > 0 || sent == total,
            "{} shrank while it was being sent; try again",
            push.name
        );
        sha.update(&buf[..n]);
        copy.write_all(&buf[..n])?;
        let frame = json!({
            "type": "attachment_push", "project_id": push.link.project_id,
            "bot_id": push.bot_id, "item_id": push.item_id, "upload_id": upload_id,
            "name": format!("upload.{}", ft.ext), "offset": sent, "total": total,
            "data": b64().encode(&buf[..n]),
        });
        let reply = app.peers.request(&push.link.peer_id, frame).await?;
        upload_id = reply["upload_id"].clone();
        sent += n as u64;
        if n == 0 {
            break;
        }
    }
    let upload_id = upload_id
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("the board's home named no upload"))?
        .to_string();
    Ok((upload_id, hex::encode(sha.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_hex_sha256_names_a_cached_file() {
        assert!(valid_sha(&"ab".repeat(32)));
        for refused in [
            "",
            "../x",
            &"AB".repeat(32),
            &"ab".repeat(33),
            &format!("{}:s", "a".repeat(62)),
        ] {
            assert!(!valid_sha(refused), "{refused}");
        }
    }

    #[test]
    fn a_cached_copy_is_used_only_while_it_hashes_to_its_record() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let path = root.join("copy.png");
        fs::write(&path, b"right bytes").unwrap();
        let sha = hex::encode(Sha256::digest(b"right bytes"));
        assert_eq!(hit(&root, &path, &sha), Some(path.clone()));
        fs::write(&path, b"other bytes").unwrap();
        assert_eq!(hit(&root, &path, &sha), None);
        assert!(!path.exists(), "a poisoned copy is removed");
        assert_eq!(hit(&root, &path, &sha), None, "nothing there");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_cache_is_never_a_hit() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let secret = outside.path().join("secret.txt");
        fs::write(&secret, b"right bytes").unwrap();
        let sha = hex::encode(Sha256::digest(b"right bytes"));
        let link = root.join("copy.txt");
        std::os::unix::fs::symlink(&secret, &link).unwrap();
        assert_eq!(hit(&root, &link, &sha), None);
        assert!(secret.exists(), "only the link goes");
    }
}

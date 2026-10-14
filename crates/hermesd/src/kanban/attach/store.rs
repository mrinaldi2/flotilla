//! Storing an attachment's bytes (0.19.0 K4b). Each file is opened once,
//! confined to the folders it may come from, and read through that one
//! handle: what is checked is what is copied, whatever happens to its path
//! meanwhile. A doc names an artifact inside the artifacts folder, or a URL.

use std::fs::{self, File, Metadata};
use std::io::{Read, Seek, SeekFrom, Write as _};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::super::attachments::{
    check_content, check_file_size, file_type, item_dir, safe_name, AttachmentLimits,
};
use super::super::model::AttachmentKind;
use crate::decisions::invalid;

/// What was stored, to record.
pub(super) struct Stored {
    pub kind: AttachmentKind,
    pub name: String,
    pub mime: &'static str,
    pub bytes: u64,
    pub sha256: String,
    pub path: String,
    pub url: String,
}

/// Open `path` to read it, but only when the file held open is the one a
/// path inside `roots` names now: a link swapped in after the caller looked
/// at the path, to a file out of `roots`, is refused. `roots` are canonical.
pub(crate) fn open_confined(path: &Path, roots: &[PathBuf]) -> anyhow::Result<File> {
    let file = open_to_read(path).map_err(|_| invalid(format!("no file {}", path.display())))?;
    let real = path
        .canonicalize()
        .map_err(|_| invalid(format!("no file {}", path.display())))?;
    anyhow::ensure!(
        roots.iter().any(|root| real.starts_with(root)),
        invalid(format!(
            "{} is outside the folders an attachment may come from",
            path.display()
        ))
    );
    let held = file.metadata()?;
    anyhow::ensure!(
        held.is_file(),
        invalid(format!("{} is not a file", path.display()))
    );
    anyhow::ensure!(
        same_file(&held, &fs::metadata(&real)?),
        invalid(format!(
            "{} changed while it was being attached; try again",
            path.display()
        ))
    );
    Ok(file)
}

/// Opens without waiting: a named pipe put where a file was expected would
/// block the open until something writes to it, holding a worker for good.
/// It opens at once instead, and is refused for not being a file. Regular
/// files ignore the flag.
#[cfg(unix)]
fn open_to_read(path: &Path) -> std::io::Result<File> {
    use std::os::unix::fs::OpenOptionsExt;
    fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
}

/// Windows has no named pipes among files: a plain open never waits.
#[cfg(not(unix))]
fn open_to_read(path: &Path) -> std::io::Result<File> {
    File::open(path)
}

#[cfg(unix)]
fn same_file(a: &Metadata, b: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}

/// Windows has no stable file id in std: the same size and times stand in.
#[cfg(not(unix))]
fn same_file(a: &Metadata, b: &Metadata) -> bool {
    a.len() == b.len()
        && a.modified().ok() == b.modified().ok()
        && a.created().ok() == b.created().ok()
}

/// A folder, canonical, for [`open_confined`]; none when it doesn't exist.
pub(crate) fn root(dir: &Path) -> Vec<PathBuf> {
    dir.canonicalize().into_iter().collect()
}

/// Copy `from` into the item's folder under its type's checks and the file
/// cap, as `<sha256 prefix>-<safe name>`. The item's own cap is the
/// caller's: `stored_on_item` is what the item holds besides this path.
pub(super) fn store(
    limits: &AttachmentLimits,
    artifacts: &Path,
    item_id: &str,
    mut from: File,
    name: &str,
    stored_on_item: &dyn Fn(&str) -> anyhow::Result<u64>,
) -> anyhow::Result<Stored> {
    let ft = file_type(limits, name)?;
    let bytes = from.metadata()?.len();
    check_file_size(limits, ft.kind, bytes)?;
    let mut head = Vec::with_capacity(16);
    (&mut from).take(16).read_to_end(&mut head)?;
    check_content(ft, &head)?;
    from.seek(SeekFrom::Start(0))?;
    let dir = item_dir(artifacts, item_id);
    fs::create_dir_all(&dir)?;
    let partial = dir.join(format!(".{}.part", bus::new_id()));
    let copied = copy_hashing(&mut from, bytes, &partial);
    drop(from);
    let sha256 = match copied {
        Ok(sha256) => sha256,
        Err(e) => {
            fs::remove_file(&partial).ok();
            return Err(e);
        }
    };
    let name = safe_name(name);
    let file_name = format!("{}-{name}", &sha256[..12]);
    let to = dir.join(&file_name);
    let rel = to
        .strip_prefix(artifacts)
        .map_err(|_| anyhow::anyhow!("an item's folder is in the artifacts"))?;
    let path = slash_path(rel);
    let fits = stored_on_item(&path)
        .and_then(|on_item| super::super::attachments::check_item_total(limits, bytes, on_item));
    if let Err(e) = fits {
        fs::remove_file(&partial).ok();
        return Err(e);
    }
    put_in_place(&partial, &to)?;
    Ok(Stored {
        kind: ft.kind,
        name,
        mime: ft.mime,
        bytes,
        sha256,
        path,
        url: String::new(),
    })
}

/// Copy exactly `bytes` bytes of `from` to `to`, returning their sha256; a
/// file that grew or shrank since it was measured is refused.
fn copy_hashing(from: &mut File, bytes: u64, to: &Path) -> anyhow::Result<String> {
    let mut out = File::create(to)?;
    let mut sha = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut copied = 0u64;
    let mut limited = from.take(bytes + 1);
    loop {
        let n = limited.read(&mut buf)?;
        if n == 0 {
            break;
        }
        copied += n as u64;
        anyhow::ensure!(
            copied <= bytes,
            invalid("the file grew while it was being attached; try again")
        );
        sha.update(&buf[..n]);
        out.write_all(&buf[..n])?;
    }
    anyhow::ensure!(
        copied == bytes,
        invalid("the file shrank while it was being attached; try again")
    );
    out.sync_all()?;
    Ok(hex::encode(sha.finalize()))
}

/// Move the copy to its name. The same bytes under the same name may be
/// there already (another attach, or the same file again); then the copy
/// goes, and so it does when Windows won't replace a file someone holds open.
fn put_in_place(partial: &Path, to: &Path) -> anyhow::Result<()> {
    if to.is_file() {
        fs::remove_file(partial).ok();
        return Ok(());
    }
    match fs::rename(partial, to) {
        Ok(()) => Ok(()),
        Err(_) if to.is_file() => {
            fs::remove_file(partial).ok();
            Ok(())
        }
        Err(e) => {
            fs::remove_file(partial).ok();
            Err(e.into())
        }
    }
}

/// A relative path with `/` between its parts, on every platform.
fn slash_path(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// A project artifact, by its path in the artifacts folder. The path stays
/// in the folder: no `..`, no drive or stream (`:`), and no link out of it.
pub(super) fn doc_artifact(
    artifacts: &Path,
    rel: &str,
    named: Option<&str>,
) -> anyhow::Result<Stored> {
    let parts: Vec<&str> = rel.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    anyhow::ensure!(
        !parts.is_empty()
            && !parts
                .iter()
                .any(|p| *p == ".." || *p == "." || p.contains(':')),
        invalid("name an artifact by its path inside the artifacts folder")
    );
    let path = parts
        .iter()
        .fold(artifacts.to_path_buf(), |dir, p| dir.join(p));
    anyhow::ensure!(path.is_file(), invalid(format!("no artifact {rel}")));
    let file = open_confined(&path, &root(artifacts))
        .map_err(|_| invalid(format!("{rel} is outside the artifacts folder")))?;
    let name = named.unwrap_or(parts[parts.len() - 1]);
    Ok(Stored {
        kind: AttachmentKind::Doc,
        name: name.to_string(),
        mime: "",
        bytes: file.metadata()?.len(),
        sha256: String::new(),
        path: parts.join("/"),
        url: String::new(),
    })
}

pub(super) fn doc_url(url: &str, named: Option<&str>) -> anyhow::Result<Stored> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_opens_only_inside_its_roots() {
        let inside = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let file = inside.path().join("a.txt");
        fs::write(&file, b"hi").unwrap();
        let roots = root(inside.path());
        assert!(open_confined(&file, &roots).is_ok());
        let other = outside.path().join("b.txt");
        fs::write(&other, b"secret").unwrap();
        assert!(open_confined(&other, &roots).is_err());
        assert!(open_confined(inside.path(), &roots).is_err(), "a folder");
        #[cfg(unix)]
        {
            let link = inside.path().join("link.txt");
            std::os::unix::fs::symlink(&other, &link).unwrap();
            let refused = open_confined(&link, &roots).expect_err("a link out");
            assert!(refused.to_string().contains("outside"), "{refused}");
        }
    }

    /// A pipe named like a file must not hang the daemon: opening it to
    /// read would wait for a writer that never comes (0.19.0 K6 review).
    #[cfg(unix)]
    #[test]
    fn a_named_pipe_is_refused_without_waiting_for_a_writer() {
        use std::os::unix::ffi::OsStrExt;
        let dir = tempfile::tempdir().unwrap();
        let pipe = dir.path().join("shot.png");
        let c_path = std::ffi::CString::new(pipe.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        let roots = root(dir.path());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(open_confined(&pipe, &roots).map(|_| ()));
        });
        let opened = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("open_confined waited for a writer to the pipe");
        let refused = opened.expect_err("a pipe is not a file");
        assert!(refused.to_string().contains("not a file"), "{refused}");
    }

    #[test]
    fn an_artifact_names_no_drive_stream_or_parent() {
        let artifacts = tempfile::tempdir().unwrap();
        fs::write(artifacts.path().join("notes.md"), b"# notes\n").unwrap();
        assert!(doc_artifact(artifacts.path(), "notes.md", None).is_ok());
        for refused in ["C:/Windows/win.ini", "notes.md:hidden", "../notes.md", "."] {
            assert!(
                doc_artifact(artifacts.path(), refused, None).is_err(),
                "{refused}"
            );
        }
    }

    #[test]
    fn a_copy_is_exactly_the_measured_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("from.txt");
        fs::write(&from, b"0123456789").unwrap();
        let to = dir.path().join("to.part");
        let mut file = File::open(&from).unwrap();
        assert!(copy_hashing(&mut file, 4, &to).is_err(), "it grew");
        let mut file = File::open(&from).unwrap();
        assert!(copy_hashing(&mut file, 20, &to).is_err(), "it shrank");
        let mut file = File::open(&from).unwrap();
        let sha = copy_hashing(&mut file, 10, &to).unwrap();
        assert_eq!(fs::read(&to).unwrap(), b"0123456789");
        assert_eq!(sha.len(), 64);
    }
}

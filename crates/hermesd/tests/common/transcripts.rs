//! Where Claude Code keeps a workspace's transcripts, written out
//! independently of the daemon's reader so the tests check it.

use std::path::{Path, PathBuf};

/// `~/.claude/projects/<key>`, where the key is the workspace's absolute path
/// with every character that is not an ASCII letter or digit replaced by
/// `-`. Not just separators: a Windows runner's temp dir is an 8.3 short
/// path (`C:\Users\RUNNER~1\…`), whose `~` becomes `-` too.
pub fn transcript_dir(user_home: &Path, workspace: impl AsRef<Path>) -> PathBuf {
    let key: String = workspace
        .as_ref()
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    user_home.join(".claude").join("projects").join(key)
}

//! The database half of the home migration: absolute paths under the old home
//! become paths under the new one, in one transaction.
//!
//! `bot.workspace_path` is what the supervisor spawns sessions in, so it has
//! to be right. The free-text columns that can embed a home path (bot
//! instructions and their revision history, routine prompts, worker briefs,
//! a local project repo) are rewritten too, so nothing keeps pointing at the
//! compatibility symlink once it is removed. Message and decision text is
//! history and stays as it was written.

use std::path::Path;

use anyhow::Context;
use rusqlite::{params, Connection, OptionalExtension};

/// `(table, id column, text columns)` rewritten by the migration.
const COLUMNS: &[(&str, &str, &[&str])] = &[
    (
        "bot",
        "id",
        &["workspace_path", "instructions", "description"],
    ),
    ("bot_revision", "id", &["old_value", "new_value"]),
    ("routine", "id", &["prompt"]),
    ("worker", "id", &["brief", "instructions"]),
    ("project_repo", "project_id", &["url"]),
];

pub const MIGRATED_KEY: &str = "home_migrated";
pub const MIGRATED_FROM_KEY: &str = "home_migrated_from";
/// Where the last move went. Moves before Flotilla's set only the two keys
/// above, so a database marked by the Gravity → The Hermes move does not
/// count as moved to `~/.flotilla`.
pub const MIGRATED_TO_KEY: &str = "home_migrated_to";
/// Every move so far, oldest first, as a JSON array of `{from, to}`.
pub const MIGRATIONS_KEY: &str = "home_migrations";

/// The `meta` rows a move writes, and a rollback puts back as they were.
const META_KEYS: [&str; 4] = [
    MIGRATED_KEY,
    MIGRATED_FROM_KEY,
    MIGRATED_TO_KEY,
    MIGRATIONS_KEY,
];

/// The `meta` rows of [`META_KEYS`] before a move: `(key, value)`, `None`
/// for a row that was absent.
pub type MetaSnapshot = Vec<(String, Option<String>)>;

/// What [`apply`] leaves in `meta`.
pub enum Mark<'a> {
    /// A finished move: the keys set for this one, the history extended.
    Migrated,
    /// A rollback: the rows as the snapshot had them. `None` (a move logged
    /// before snapshots existed) removes the two keys that move set.
    Restore(Option<&'a MetaSnapshot>),
}

/// Whether paths compare as Windows compares them: case-insensitive, with
/// `/` and `\` alike. Agents write `C:/Users/x/.gravity` as often as the
/// backslash form.
const LOOSE: bool = cfg!(windows);

/// Replaces `from` with `to` wherever it appears as a whole path or a path
/// prefix: followed by a separator, or at the very end of the text.
pub fn rewrite(text: &str, from: &str, to: &str) -> String {
    rewrite_as(text, from, to, LOOSE)
}

/// [`rewrite`], comparing loosely or exactly. A loose match spelled with
/// forward slashes gets `to` with forward slashes too.
pub fn rewrite_as(text: &str, from: &str, to: &str, loose: bool) -> String {
    if from.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    while let Some((start, end)) = find(text, from, copied, loose) {
        let after = &text[end..];
        let boundary = after.is_empty() || after.starts_with('/') || after.starts_with('\\');
        out.push_str(&text[copied..start]);
        let found = &text[start..end];
        if !boundary {
            out.push_str(found);
        } else if found.contains('/') && !found.contains('\\') {
            out.push_str(&to.replace('\\', "/"));
        } else {
            out.push_str(to);
        }
        copied = end;
    }
    out.push_str(&text[copied..]);
    out
}

/// The byte range of the next `from` in `text` at or after `at`.
fn find(text: &str, from: &str, at: usize, loose: bool) -> Option<(usize, usize)> {
    if !loose {
        return text[at..].find(from).map(|i| (at + i, at + i + from.len()));
    }
    text[at..].char_indices().find_map(|(i, _)| {
        let start = at + i;
        prefix_len(&text[start..], from).map(|len| (start, start + len))
    })
}

/// How many bytes of `text` loosely spell `from`, if it starts with it.
fn prefix_len(text: &str, from: &str) -> Option<usize> {
    let fold = |c: char| match c {
        '/' => '\\',
        c => c.to_lowercase().next().unwrap_or(c),
    };
    let mut chars = text.char_indices();
    for wanted in from.chars() {
        let (_, got) = chars.next()?;
        if fold(got) != fold(wanted) {
            return None;
        }
    }
    Some(chars.next().map_or(text.len(), |(i, _)| i))
}

/// Whether `path` is `home` or under it, in any spelling: case and either
/// separator, whatever the platform.
pub fn is_under(path: &str, home: &str) -> bool {
    prefix_len(path, home).is_some_and(|len| {
        let rest = &path[len..];
        rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\')
    })
}

/// `C:\Users\x` as Git Bash and MSYS tools write it: `/c/Users/x`.
pub fn msys_spelling(path: &str) -> Option<String> {
    let mut chars = path.chars();
    let drive = chars.next().filter(char::is_ascii_alphabetic)?;
    if chars.next() != Some(':') {
        return None;
    }
    Some(format!(
        "/{}{}",
        drive.to_ascii_lowercase(),
        chars.as_str().replace('\\', "/")
    ))
}

/// Local, live bots whose workspace still sits under one of `homes` after
/// the rewrite. Such a bot would start under the old path, and so under
/// the old transcript key, losing its history without an error.
fn stale_workspaces(conn: &Connection, homes: &[&str]) -> anyhow::Result<Vec<String>> {
    let mut filters = Vec::new();
    for (column, filter) in [
        ("deleted_at", "deleted_at IS NULL"),
        ("peer_id", "peer_id IS NULL"),
    ] {
        if has_column(conn, "bot", column)? {
            filters.push(filter);
        }
    }
    let filter = if filters.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", filters.join(" AND "))
    };
    let mut stmt = conn.prepare(&format!("SELECT name, workspace_path FROM bot{filter}"))?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter(|(_, path)| homes.iter().any(|home| is_under(path, home)))
        .map(|(name, path)| format!("{name} ({path})"))
        .collect())
}

fn has_column(tx: &Connection, table: &str, column: &str) -> anyhow::Result<bool> {
    let mut stmt = tx.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names.iter().any(|name| name == column))
}

/// How many values each `(from, to)` pair would change, per `table.column`.
/// Read-only; used by the dry run.
pub fn count(db: &Path, pairs: &[(String, String)]) -> anyhow::Result<Vec<(String, usize)>> {
    let conn = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening {}", db.display()))?;
    let mut counts = Vec::new();
    for (table, id, columns) in COLUMNS {
        for column in *columns {
            if !has_column(&conn, table, column)? {
                continue;
            }
            let rows = changed_rows(&conn, table, id, column, pairs)?;
            if !rows.is_empty() {
                counts.push((format!("{table}.{column}"), rows.len()));
            }
        }
    }
    Ok(counts)
}

fn changed_rows(
    conn: &Connection,
    table: &str,
    id: &str,
    column: &str,
    pairs: &[(String, String)],
) -> anyhow::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare(&format!("SELECT {id}, {column} FROM {table}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, value)| {
            let value = value?;
            let new = pairs
                .iter()
                .fold(value.clone(), |text, (from, to)| rewrite(&text, from, to));
            (new != value).then_some((id, new))
        })
        .collect())
}

/// The [`META_KEYS`] rows in `db` now.
pub fn meta_snapshot(db: &Path) -> anyhow::Result<MetaSnapshot> {
    let conn = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening {}", db.display()))?;
    META_KEYS
        .iter()
        .map(|key| Ok((key.to_string(), meta_value(&conn, key)?)))
        .collect()
}

fn meta_value(conn: &Connection, key: &str) -> anyhow::Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM meta WHERE key = ?1", params![key], |r| {
            r.get(0)
        })
        .optional()?)
}

fn set_meta(tx: &Connection, key: &str, value: &str) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![key, value],
    )?;
    Ok(())
}

/// The move history with `from → to` appended. A database marked by a move
/// from before the history existed starts it with that move.
fn history(tx: &Connection, from: &str, to: &str) -> anyhow::Result<String> {
    let mut moves: Vec<serde_json::Value> = meta_value(tx, MIGRATIONS_KEY)?
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    if moves.is_empty() {
        if let Some(earlier) = meta_value(tx, MIGRATED_FROM_KEY)? {
            moves.push(serde_json::json!({ "from": earlier, "to": from }));
        }
    }
    let this = serde_json::json!({ "from": from, "to": to });
    // A run that committed but died before recording the step does it again.
    if moves.last() != Some(&this) {
        moves.push(this);
    }
    Ok(serde_json::Value::Array(moves).to_string())
}

/// Rewrites every column in one transaction and marks `meta` (see [`Mark`])
/// so the migration can tell it already ran. `pairs` lists `(from, to)`
/// spellings, e.g. the home as configured and as resolved through symlinks.
pub fn apply(db: &Path, pairs: &[(String, String)], mark: Mark<'_>) -> anyhow::Result<usize> {
    let mut conn = Connection::open(db).with_context(|| format!("opening {}", db.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(10))?;
    let tx = conn.transaction()?;
    let mut changed = 0;
    for (table, id, columns) in COLUMNS {
        for column in *columns {
            if !has_column(&tx, table, column)? {
                continue;
            }
            for (key, value) in changed_rows(&tx, table, id, column, pairs)? {
                tx.execute(
                    &format!("UPDATE {table} SET {column} = ?1 WHERE {id} = ?2"),
                    params![value, key],
                )?;
                changed += 1;
            }
        }
    }
    match mark {
        Mark::Migrated => {
            let homes: Vec<&str> = pairs.iter().map(|(from, _)| from.as_str()).collect();
            let stale = stale_workspaces(&tx, &homes)?;
            anyhow::ensure!(
                stale.is_empty(),
                "bot workspaces still under the old home after the rewrite: {}; \
                 fix those paths and retry",
                stale.join(", ")
            );
            let (from, to) = pairs
                .first()
                .map(|(from, to)| (from.as_str(), to.as_str()))
                .unwrap_or(("", ""));
            // Read before the keys below change; a resumed run that already
            // committed never gets here twice (the step is then done).
            let moves = history(&tx, from, to)?;
            for (key, value) in [
                (MIGRATED_KEY, "1"),
                (MIGRATED_FROM_KEY, from),
                (MIGRATED_TO_KEY, to),
                (MIGRATIONS_KEY, moves.as_str()),
            ] {
                set_meta(&tx, key, value)?;
            }
        }
        Mark::Restore(Some(snapshot)) => {
            for key in META_KEYS {
                tx.execute("DELETE FROM meta WHERE key = ?1", params![key])?;
            }
            for (key, value) in snapshot {
                if let Some(value) = value {
                    set_meta(&tx, key, value)?;
                }
            }
        }
        Mark::Restore(None) => {
            tx.execute(
                "DELETE FROM meta WHERE key IN (?1, ?2)",
                params![MIGRATED_KEY, MIGRATED_FROM_KEY],
            )?;
        }
    }
    tx.commit()?;
    Ok(changed)
}

/// Bots the daemon runs here: live and not linked from a peer.
pub fn live_bots(db: &Path) -> anyhow::Result<usize> {
    let conn = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut filters = vec!["1"];
    for (column, filter) in [
        ("deleted_at", "deleted_at IS NULL"),
        ("peer_id", "peer_id IS NULL"),
    ] {
        if has_column(&conn, "bot", column)? {
            filters.push(filter);
        }
    }
    let sql = format!("SELECT COUNT(*) FROM bot WHERE {}", filters.join(" AND "));
    Ok(conn.query_row(&sql, [], |r| r.get::<_, i64>(0))? as usize)
}

/// Whether the database at `db` records a finished move to `home`: an
/// earlier move's `meta.home_migrated` alone does not count.
pub fn is_marked_to(db: &Path, home: &str) -> anyhow::Result<bool> {
    let conn = Connection::open_with_flags(db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let marked = meta_value(&conn, MIGRATED_KEY)?.as_deref() == Some("1");
    let to = meta_value(&conn, MIGRATED_TO_KEY)?;
    Ok(marked && to.is_some_and(|to| is_under(&to, home) && is_under(home, &to)))
}

#[cfg(test)]
#[path = "sql_tests.rs"]
mod tests;

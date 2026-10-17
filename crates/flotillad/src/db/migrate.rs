//! Applying `MIGRATIONS` in order. A run of ordinary migrations goes in one
//! transaction. One whose first line is `-- foreign_keys: off` rebuilds a
//! table other tables reference, so it runs alone with foreign keys off
//! (SQLite's procedure for changing a table's schema). It is refused if it
//! leaves more broken references than there were before.

use anyhow::Context;
use bus::schema::MIGRATIONS;
use rusqlite::{params, Connection, Transaction};

use super::{migrate_columns, schema_version};

const FOREIGN_KEYS_OFF: &str = "-- foreign_keys: off";

fn needs_foreign_keys_off(sql: &str) -> bool {
    sql.starts_with(FOREIGN_KEYS_OFF)
}

pub(super) fn migrate(conn: &mut Connection) -> anyhow::Result<()> {
    loop {
        let version = schema_version(conn)?;
        let Some(next) = MIGRATIONS.get(version) else {
            return Ok(());
        };
        if needs_foreign_keys_off(next) {
            without_foreign_keys(conn, version)?;
            continue;
        }
        let tx = conn.transaction()?;
        let mut at = version;
        while let Some(sql) = MIGRATIONS
            .get(at)
            .filter(|sql| !needs_foreign_keys_off(sql))
        {
            apply(&tx, at, sql)?;
            at += 1;
        }
        set_version(&tx, at)?;
        tx.commit()?;
    }
}

fn apply(tx: &Transaction<'_>, index: usize, sql: &str) -> anyhow::Result<()> {
    let sql = migrate_columns::without_existing_columns(tx, sql)?;
    tx.execute_batch(&sql)
        .with_context(|| format!("applying migration {}", index + 1))
}

fn set_version(tx: &Transaction<'_>, version: usize) -> anyhow::Result<()> {
    tx.execute(
        "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = ?1",
        params![version.to_string()],
    )?;
    Ok(())
}

fn broken_references(conn: &Connection) -> anyhow::Result<i64> {
    Ok(
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })?,
    )
}

fn without_foreign_keys(conn: &mut Connection, index: usize) -> anyhow::Result<()> {
    conn.pragma_update(None, "foreign_keys", "OFF")?;
    let applied = (|| -> anyhow::Result<()> {
        let before = broken_references(conn)?;
        let tx = conn.transaction()?;
        apply(&tx, index, MIGRATIONS[index])?;
        let after = broken_references(&tx)?;
        anyhow::ensure!(
            after <= before,
            "migration {} would leave {} broken references (there were {before})",
            index + 1,
            after
        );
        set_version(&tx, index + 1)?;
        tx.commit()?;
        Ok(())
    })();
    conn.pragma_update(None, "foreign_keys", "ON")?;
    applied
}

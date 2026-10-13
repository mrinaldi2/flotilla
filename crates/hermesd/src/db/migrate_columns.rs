//! SQLite has no `ADD COLUMN IF NOT EXISTS`. Before a migration runs, each
//! of its `ALTER TABLE <t> ADD COLUMN <c> …;` statements whose column already
//! exists is dropped, so a migration that adds columns stays safe to run
//! again, as every migration here is (tests rewind the schema version and
//! replay what follows).

use rusqlite::{params, Connection};

pub(super) fn without_existing_columns(conn: &Connection, sql: &str) -> rusqlite::Result<String> {
    let mut out = String::with_capacity(sql.len());
    let mut rest = sql;
    while let Some(at) = next_add_column(rest) {
        let (before, from) = rest.split_at(at);
        out.push_str(before);
        let end = from.find(';').map_or(from.len(), |i| i + 1);
        let (statement, after) = from.split_at(end);
        // ALTER TABLE <table> ADD COLUMN <column> …
        let mut words = statement.split_whitespace();
        let table = words.nth(2);
        let column = words.nth(2);
        let exists = match (table, column) {
            (Some(table), Some(column)) => has_column(conn, table, column)?,
            _ => false,
        };
        if !exists {
            out.push_str(statement);
        }
        rest = after;
    }
    out.push_str(rest);
    Ok(out)
}

/// Where the next line starting `ALTER TABLE … ADD COLUMN` begins.
fn next_add_column(sql: &str) -> Option<usize> {
    let mut at = 0;
    for line in sql.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("ALTER TABLE ") && trimmed.contains(" ADD COLUMN ") {
            return Some(at + line.len() - trimmed.len());
        }
        at += line.len();
    }
    None
}

fn has_column(conn: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    conn.query_row(
        "SELECT count(*) FROM pragma_table_info(?1) WHERE name = ?2",
        params![table, column],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_existing_column_is_not_added_again() {
        let conn = Connection::open_in_memory().expect("db");
        conn.execute_batch("CREATE TABLE t (a TEXT);")
            .expect("table");
        let sql = "ALTER TABLE t ADD COLUMN a TEXT;\nALTER TABLE t ADD COLUMN b TEXT;\nSELECT 1;";
        let kept = without_existing_columns(&conn, sql).expect("rewrite");
        assert_eq!(kept, "\nALTER TABLE t ADD COLUMN b TEXT;\nSELECT 1;");
        conn.execute_batch(&kept).expect("runs");
        assert_eq!(
            without_existing_columns(&conn, sql).expect("again"),
            "\n\nSELECT 1;"
        );
    }
}

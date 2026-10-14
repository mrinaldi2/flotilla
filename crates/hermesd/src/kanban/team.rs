//! The team's setup on a board: how much each column may hold, set by the
//! owner in the app. Roles are the project's (`crate::project_roles`). Each
//! change is pushed, so open boards refetch.

use std::sync::Arc;

use super::feed::{Change, ChangeKind};
use crate::app::AppState;

/// Sets a column's WIP limit, or clears it. Zero is no limit too.
pub fn set_column_limit(
    app: &Arc<AppState>,
    project_id: &str,
    column_key: &str,
    limit: Option<u32>,
) -> anyhow::Result<()> {
    let limit = limit.filter(|l| *l > 0);
    let mut feed = app.board.writer();
    anyhow::ensure!(
        app.db.set_column_limit(project_id, column_key, limit)?,
        "no column '{column_key}' on this board"
    );
    feed.publish(changed(project_id, ChangeKind::ColumnsChanged));
    Ok(())
}

fn changed(project_id: &str, kind: ChangeKind) -> Change<'_> {
    Change {
        project_id,
        kind,
        item_id: "",
        card: None,
        from_column: None,
    }
}

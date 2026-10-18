//! What the plan's kickoffs read and write inside the planner's
//! transaction (0.21.0): the project's lead, a node's open kickoff cards,
//! and the column a finished one goes to.

use rusqlite::{params, OptionalExtension};

use super::BoardTx;

/// A live card of a plan node, by title, with where it stands.
#[derive(Debug, Clone)]
pub struct NodeCard {
    pub id: String,
    pub title: String,
    pub version: u64,
    pub board_id: String,
    /// Done or cancelled.
    pub closed: bool,
}

impl BoardTx<'_> {
    /// The project's lead bot: the one the project names, else a bot with
    /// the board's lead role.
    pub fn plan_project_lead(&self, project_id: &str) -> anyhow::Result<Option<String>> {
        let named: Option<String> = self
            .conn
            .query_row(
                "SELECT p.lead_bot_id FROM project p JOIN bot b ON b.id = p.lead_bot_id
                 WHERE p.id = ?1 AND b.deleted_at IS NULL",
                params![project_id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if named.is_some() {
            return Ok(named);
        }
        Ok(self
            .conn
            .query_row(
                "SELECT r.bot_id FROM project_role r JOIN bot b ON b.id = r.bot_id
                 WHERE r.project_id = ?1 AND r.role = 'lead' AND b.deleted_at IS NULL
                 ORDER BY r.bot_id LIMIT 1",
                params![project_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// The node's live cards.
    pub fn plan_node_cards(&self, node_id: &str) -> anyhow::Result<Vec<NodeCard>> {
        let cards = self
            .conn
            .prepare(
                "SELECT id, title, version, coalesce(board_id, ''),
                        category IN ('done', 'cancelled')
                 FROM item WHERE plan_node_id = ?1 AND state = 'live' ORDER BY seq",
            )?
            .query_map(params![node_id], |r| {
                Ok(NodeCard {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    version: r.get(2)?,
                    board_id: r.get(3)?,
                    closed: r.get(4)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(cards)
    }

    /// The board's first done column.
    pub fn done_column_of(&self, board_id: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT key FROM board_column WHERE board_id = ?1 AND category = 'done'
                 ORDER BY ord LIMIT 1",
                params![board_id],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// The project's default board.
    pub fn default_board_of(&self, project_id: &str) -> anyhow::Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM board WHERE project_id = ?1 AND is_default = 1
                 AND archived_at IS NULL",
                params![project_id],
                |r| r.get(0),
            )
            .optional()?)
    }
}

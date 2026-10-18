//! The board's read requests, each from one consistent snapshot.

use std::collections::HashSet;

use bus::contract::board as c;
use bus::Capability;

use super::{not_found, refuse, Refusal};
use crate::app::AppState;
use crate::kanban::attachments::to_wire;
use crate::kanban::contract::MapError;
use crate::kanban::model as m;
use crate::kanban::moves;
use crate::kanban::presets::{Preset, Start};
use crate::ws::Conn;

/// History events per page when the client names no limit, and the most it
/// may ask for.
const HISTORY_PAGE: u32 = 100;
const HISTORY_MAX: u32 = 500;

impl Conn {
    /// The whole board, current with the project's push `seq`, enabling it
    /// first when `board_get` may (see `enable_board`).
    pub(super) fn snapshot(&self, project_id: &str) -> Result<c::BoardSnapshot, Refusal> {
        let mut board = match snapshot(&self.app, project_id)? {
            Some(board) => board,
            // Its home is a peer: the board as last mirrored, read-only (B9).
            None if self.app.board_mirror.home_peer(project_id).is_some() => {
                return self.mirrored_snapshot(project_id)
            }
            None => {
                self.enable_board(project_id, Enable::Auto, &Start::of(Preset::FIRST))?;
                snapshot(&self.app, project_id)?
                    .ok_or_else(|| refuse("internal", "the board was enabled but is missing"))?
            }
        };
        // A WS connection is the owner's (token or device): the grant decides.
        board.can_rule = self.caps.contains(&Capability::Approve);
        board.drafts = self
            .app
            .db
            .board_read(|t| t.draft_cards(project_id))?
            .into_iter()
            .map(Into::into)
            .collect();
        board.proposals = super::structure::proposals(&self.app, project_id)?;
        Ok(board)
    }

    /// Enables a project's board here with the B2 defaults, recording this
    /// daemon as its home (H-037). `board_get` does it on its own only for
    /// a project with no link: which computer of a linked team is the home
    /// is the owner's call, made with `board_enable`.
    pub(super) fn enable_board(
        &self,
        project_id: &str,
        how: Enable,
        start: &Start,
    ) -> Result<(), Refusal> {
        let db = &self.app.db;
        if !db
            .get_project(project_id)?
            .is_some_and(|p| p.deleted_at.is_none())
        {
            return Err(refuse("not_found", format!("no project {project_id}")));
        }
        if how == Enable::Auto {
            if !self.caps.contains(&Capability::Control) {
                return Err(refuse(
                    "no_board",
                    "This project has no board yet; enabling it needs the control grant.",
                ));
            }
            if !db.project_links(project_id)?.is_empty() {
                return Err(refuse(
                    "no_board",
                    "This project is linked with another computer, so its board isn't \
                     started on its own. Start it on the computer that should be its home.",
                ));
            }
        }
        db.ensure_board_from(project_id, &db.daemon_id()?, None, start)?;
        Ok(())
    }

    pub(super) fn item_get(&self, id: &str) -> Result<c::ItemDetail, Refusal> {
        item_detail(&self.app, id)
    }

    pub(super) fn item_history(&self, r: &c::ItemHistory) -> Result<c::ItemHistoryPage, Refusal> {
        history_page(&self.app, r)
    }

    pub(super) fn item_query(&self, r: &c::ItemQuery) -> Result<c::ItemQueryResult, Refusal> {
        let text = r.text.as_deref().map(str::trim).filter(|t| !t.is_empty());
        let cards = self.app.db.board_read(|t| match text {
            Some(text) => t.search_cards(&r.project_id, text),
            None => t.cards(&r.project_id),
        });
        let cards = match (cards, text) {
            (Ok(cards), _) => cards,
            (Err(e), Some(_)) => return Err(refuse("invalid_request", format!("search: {e}"))),
            (Err(e), None) => return Err(e.into()),
        };
        let nodes = crate::plan::subtree_ids(&self.app, &r.plan_node_ids)?;
        query_cards(r, cards, nodes.as_ref())
    }
}

/// The cards among `cards` an `ItemQuery` asks for, its text aside.
/// `nodes` are the plan nodes it asks for with every node under them.
pub(super) fn query_cards(
    r: &c::ItemQuery,
    cards: Vec<m::ItemCard>,
    nodes: Option<&HashSet<String>>,
) -> Result<c::ItemQueryResult, Refusal> {
    let bad = |e: MapError| refuse("invalid_request", e.to_string());
    let types = wire_list(&r.types, m::ItemType::from_wire).map_err(bad)?;
    let priorities = wire_list(&r.priorities, m::Priority::from_wire).map_err(bad)?;
    let labels = crate::compat018::query_labels(r);
    let wanted = |card: &m::ItemCard| {
        (r.column_keys.is_empty() || r.column_keys.contains(&card.column_key))
            && r.assignee
                .as_ref()
                .is_none_or(|a| card.assignee.as_ref() == Some(a))
            && (types.is_empty() || types.contains(&card.item_type))
            && (priorities.is_empty() || priorities.contains(&card.priority))
            && crate::kanban::labels::matches_any(&card.labels, &labels)
            && r.blocked.is_none_or(|b| card.blocked == b)
            && nodes.is_none_or(|n| card.plan_node_id.as_ref().is_some_and(|id| n.contains(id)))
    };
    Ok(c::ItemQueryResult {
        cards: cards.into_iter().filter(wanted).map(Into::into).collect(),
    })
}

impl Conn {
    pub(super) fn item_move_check(&self, id: &str) -> Result<c::MoveCheck, Refusal> {
        move_check(&self.app, id, &self.actor())
    }
}

/// One item with its links, comments and first page of history.
/// Its attachments carry this computer's paths, and a draft names the bots
/// invited to it.
pub(super) fn item_detail(app: &AppState, id: &str) -> Result<c::ItemDetail, Refusal> {
    let project_id = app
        .db
        .board_read(|t| t.item_project(id))?
        .ok_or_else(|| not_found(id))?;
    let artifacts = crate::kanban::attach::artifacts_of(app, &project_id)?;
    app.db
        .board_read(|t| {
            let Some(item) = t.item(id)? else {
                return Ok(None);
            };
            let (history, next) = t.item_history(id, None, HISTORY_PAGE)?;
            Ok(Some(c::ItemDetail {
                item: Some(item.into()),
                links: t.item_links(id)?.into_iter().map(Into::into).collect(),
                comments: t.item_comments(id)?.into_iter().map(Into::into).collect(),
                history: history.into_iter().map(Into::into).collect(),
                history_next: next,
                attachments: t
                    .attachments(id)?
                    .into_iter()
                    .map(|a| to_wire(a, &artifacts))
                    .collect(),
                invited: t.invited(id)?,
                // Filled with the questions (0.22.0, step 2).
                questions: Vec::new(),
            }))
        })?
        .ok_or_else(|| not_found(id))
}

pub(super) fn history_page(
    app: &AppState,
    r: &c::ItemHistory,
) -> Result<c::ItemHistoryPage, Refusal> {
    let limit = match r.limit {
        0 => HISTORY_PAGE,
        n => n.min(HISTORY_MAX),
    };
    let (events, next) = app
        .db
        .board_read(|t| {
            if t.item_project(&r.id)?.is_none() {
                return Ok(None);
            }
            t.item_history(&r.id, r.after, limit).map(Some)
        })?
        .ok_or_else(|| not_found(&r.id))?;
    Ok(c::ItemHistoryPage {
        item_id: r.id.clone(),
        events: events.into_iter().map(Into::into).collect(),
        next,
    })
}

pub(super) fn move_check(
    app: &AppState,
    id: &str,
    actor: &crate::actor::Actor<'_>,
) -> Result<c::MoveCheck, Refusal> {
    if app.db.board_read(|t| t.item_project(id))?.is_none() {
        return Err(not_found(id));
    }
    let columns = moves::item_move_check(&app.db, id, actor)?;
    // The owner's override of a release-gated board's usual path.
    let overrides = match actor.is_owner() {
        true => moves::owner_overrides(&app.db, id)?,
        false => Vec::new(),
    };
    Ok(c::MoveCheck {
        item_id: id.to_string(),
        columns: columns
            .into_iter()
            .map(|(column, unmet)| c::ColumnCheck {
                owner_override: overrides.contains(&column.key),
                column_key: column.key,
                unmet: unmet.into_iter().map(Into::into).collect(),
            })
            .collect(),
    })
}

/// Who enables a board: `board_get` on its own, or the owner on purpose.
/// The whole board, or none before it is enabled. The push `seq` is read
/// first: a change landing in between is in the snapshot and comes again as
/// a push, which the client applies harmlessly.
pub(super) fn snapshot(
    app: &AppState,
    project_id: &str,
) -> Result<Option<c::BoardSnapshot>, Refusal> {
    let seq = app.board.seq(project_id);
    let Some(snapshot) = app.db.board_read(|t| t.snapshot(project_id))? else {
        return Ok(None);
    };
    served_here(app, &snapshot.settings.home_daemon_id)?;
    Ok(Some(c::BoardSnapshot {
        settings: Some(snapshot.settings.into()),
        columns: snapshot.columns.into_iter().map(Into::into).collect(),
        cards: snapshot.cards.into_iter().map(Into::into).collect(),
        roles: snapshot.roles.into_iter().map(Into::into).collect(),
        seq,
        // Set by the connection, which knows its grants.
        can_rule: false,
        boards: app
            .db
            .boards(project_id)?
            .into_iter()
            .map(Into::into)
            .collect(),
        // The owner's connection adds them; a linked computer never sees one.
        drafts: Vec::new(),
        proposals: Vec::new(),
    }))
}

/// One home per board (H-020 §1.3, H-037): only the daemon named in
/// `home_daemon_id` serves it.
fn served_here(app: &AppState, home_daemon_id: &str) -> Result<(), Refusal> {
    let db = &app.db;
    if home_daemon_id == db.daemon_id()? {
        return Ok(());
    }
    let home = db
        .list_peers()?
        .into_iter()
        .find(|p| p.daemon_id.as_deref() == Some(home_daemon_id))
        .map_or_else(|| "another computer".to_string(), |p| p.name);
    Err(refuse(
        "no_board",
        format!("This project's board lives on {home}; open it there."),
    ))
}

#[derive(PartialEq, Eq)]
pub(super) enum Enable {
    Auto,
    Owner,
}

fn wire_list<T>(
    numbers: &[i32],
    from_wire: impl Fn(i32) -> Result<T, MapError>,
) -> Result<Vec<T>, MapError> {
    numbers.iter().map(|n| from_wire(*n)).collect()
}

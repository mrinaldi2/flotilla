//! What the keeper reads for one project in a pass: its live cards with
//! their history times, its marks, its boards and columns, and the cards a
//! refused move still stops. Turned into each card's `Facts` here.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};

use super::memory::Keeper;
use super::rules::{Facts, Role};
use crate::app::AppState;
use crate::db::{KeeperCard, KeeperMark};
use crate::kanban::facts::Fact;
use crate::kanban::model::{Board, BoardColumn, BoardFlow, ColumnCategory, ColumnKind};
use crate::kanban::notice_text::Plan;

/// One board of the project, as the keeper reads it.
pub(super) struct BoardView {
    pub board: Option<Board>,
    pub custom: bool,
    /// The work column's place: where `work.started` moves a card, else the
    /// first active column.
    pub start_ord: Option<u32>,
    pub plan: Option<Plan>,
}

pub(super) struct Project {
    pub id: String,
    pub cards: Vec<KeeperCard>,
    pub marks: Vec<KeeperMark>,
    pub columns: Vec<BoardColumn>,
    pub boards: HashMap<String, BoardView>,
    pub stale_after: Duration,
    /// Card → "<column>: <rule>" of a refused move that would still be.
    pub refused: HashMap<String, String>,
}

fn view(board: Option<Board>, columns: &[BoardColumn]) -> BoardView {
    let custom = board.as_ref().is_some_and(|b| b.flow == BoardFlow::Custom);
    let id = board.as_ref().map(|b| b.id.clone()).unwrap_or_default();
    let mut active: Vec<&BoardColumn> = columns
        .iter()
        .filter(|c| (id.is_empty() || c.board_id == id) && c.kind == ColumnKind::Active)
        .collect();
    active.sort_by_key(|c| c.ord);
    let mapped = board
        .as_ref()
        .and_then(|b| b.automations.get(Fact::WorkStarted.name()))
        .and_then(|key| active.iter().find(|c| &c.key == key));
    let start = mapped.or_else(|| active.iter().find(|c| c.visible));
    // Research (0.21.0) is the assignee's own work, as the column after it
    // is: review starts past that one.
    let start = start.map(|s| match s.category {
        ColumnCategory::Research => active
            .iter()
            .find(|c| c.ord > s.ord && c.category != ColumnCategory::Research)
            .unwrap_or(s),
        _ => s,
    });
    BoardView {
        start_ord: start.map(|c| c.ord),
        plan: Plan::of(board.as_ref(), columns),
        board,
        custom,
    }
}

/// Read the project for a pass.
pub(super) fn load(app: &AppState, project_id: &str) -> anyhow::Result<Project> {
    let db = &app.db;
    let columns = db.board_columns(project_id)?;
    let mut boards: HashMap<String, BoardView> = db
        .boards(project_id)?
        .into_iter()
        .map(|b| (b.id.clone(), view(Some(b), &columns)))
        .collect();
    // A board from before 0.19 has no row of its own: release-gated.
    boards.insert(String::new(), view(None, &columns));
    let stale_after = db
        .board_settings(project_id)?
        .map_or(24, |s| s.stale_after_hours);
    let cards = db.keeper_cards(project_id)?;
    let refused = refused(app, project_id, &cards, &columns)?;
    Ok(Project {
        id: project_id.to_string(),
        marks: db.keeper_marks(project_id)?,
        cards,
        columns,
        boards,
        stale_after: Duration::hours(i64::from(stale_after.max(1))),
        refused,
    })
}

/// The cards whose last refused move would still be refused, as the board's
/// "can't move" line shows them.
fn refused(
    app: &AppState,
    project_id: &str,
    cards: &[KeeperCard],
    columns: &[BoardColumn],
) -> anyhow::Result<HashMap<String, String>> {
    let ids: Vec<&str> = cards
        .iter()
        .filter(|c| crate::kanban::stuck::has_refusal(project_id, &c.id))
        .map(|c| c.id.as_str())
        .collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let mut shown: Vec<_> = app.db.board_read(|t| t.cards(project_id))?;
    shown.retain(|c| ids.contains(&c.id.as_str()));
    app.db
        .board_read(|t| crate::kanban::stuck::mark(t, project_id, &mut shown))?;
    Ok(shown
        .into_iter()
        .filter_map(|c| {
            let block = c.move_block.filter(|b| b.refused)?;
            let name = columns
                .iter()
                .find(|col| {
                    col.key == block.to_column
                        && (c.board_id.is_empty() || col.board_id == c.board_id)
                })
                .map_or(block.to_column.as_str(), |col| col.name.as_str());
            Some((c.id, super::text::refused_detail(name, &block.reason)))
        })
        .collect())
}

impl Project {
    pub fn board_of(&self, card: &KeeperCard) -> &BoardView {
        self.boards
            .get(&card.board_id)
            .or_else(|| self.boards.get(""))
            .expect("the board-less view is always there")
    }

    pub fn column_of(&self, card: &KeeperCard) -> Option<&BoardColumn> {
        self.columns.iter().find(|c| {
            c.key == card.column_key && (card.board_id.is_empty() || c.board_id == card.board_id)
        })
    }

    pub fn card(&self, id: &str) -> Option<&KeeperCard> {
        self.cards.iter().find(|c| c.id == id)
    }

    pub fn marks_of(&self, card: &KeeperCard) -> Vec<KeeperMark> {
        self.marks
            .iter()
            .filter(|m| m.item_id == card.id)
            .cloned()
            .collect()
    }

    /// What its column means for the keeper.
    pub fn role(&self, card: &KeeperCard) -> Role {
        let board = self.board_of(card);
        let Some(column) = self.column_of(card) else {
            return Role::Other;
        };
        if !board.custom {
            // Release-gated: only Research and Doing are the assignee's own
            // work.
            return if matches!(
                column.category,
                ColumnCategory::Research | ColumnCategory::Doing
            ) {
                Role::Work
            } else {
                Role::Other
            };
        }
        match column.kind {
            ColumnKind::Open => Role::Open,
            ColumnKind::Active => match board.start_ord {
                Some(start) if column.ord > start => Role::Review,
                _ => Role::Work,
            },
            ColumnKind::Done | ColumnKind::Cancelled => Role::Other,
        }
    }

    pub fn facts<'a>(&self, card: &'a KeeperCard, keeper: &Keeper, reachable: bool) -> Facts<'a> {
        let assignee = card.assignee.as_deref();
        let read = assignee.and_then(|bot| keeper.read_at(bot, &card.id));
        Facts {
            card,
            custom: self.board_of(card).custom,
            role: self.role(card),
            ages_after: crate::db::ages_in(card.category).then_some(self.stale_after),
            reachable,
            busy: assignee.is_some_and(|bot| keeper.busy(bot)),
            last_turn: assignee.and_then(|bot| keeper.last_turn(bot)),
            touched_at: latest(card.assignee_acted_at, read),
            refused: self.refused.get(&card.id).cloned(),
        }
    }
}

fn latest(a: Option<DateTime<Utc>>, b: Option<DateTime<Utc>>) -> Option<DateTime<Utc>> {
    a.max(b)
}

/// Which assignees a message reaches: live bots of the project, here or
/// on a linked computer, looked up once per pass.
#[derive(Default)]
pub(super) struct Reach {
    known: HashMap<String, bool>,
}

impl Reach {
    pub fn of(&mut self, app: &AppState, project_id: &str, bot_id: Option<&str>) -> bool {
        let Some(bot_id) = bot_id else {
            return false;
        };
        *self.known.entry(bot_id.to_string()).or_insert_with(
            || matches!(app.db.get_live_bot(bot_id), Ok(Some(bot)) if bot.project_id == project_id),
        )
    }
}

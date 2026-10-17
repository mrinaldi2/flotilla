//! Item edits, for bots over MCP and for the owner over WebSocket (0.19.0
//! K4b). Each guarded edit runs in one board transaction: load the item and
//! the caller's roles, check the version, run the guard, write (ARCH-R4 §3).
//! The change is pushed once it has committed, so open boards see it.
//!
//! A draft takes only field edits, links and attachments until it is
//! finalized; a discarded draft takes nothing.

use std::sync::Arc;

use bus::contract::board as c;

use crate::actor::Actor;
use crate::app::AppState;
use crate::db::{BoardTx, ItemEdit, NewItem, Write};
use crate::decisions::invalid;
use crate::kanban::feed::{card_after_commit, Change, ChangeKind};
use crate::kanban::guards::{self, unmet, Who};
use crate::kanban::model::{Item, ItemLink, ItemState, ItemType, LinkKind, Priority, Size, Unmet};
use crate::kanban::moves::load_in;

mod more;

/// What an edit came to.
#[derive(Debug)]
pub enum Edited {
    Done(Box<Item>),
    /// The guards that are not met; nothing was written.
    Refused(Vec<Unmet>),
    /// The item changed since the caller read it; this is how it is now.
    Conflict(Box<Item>),
}

/// Who edits, and in which project.
pub struct Editor<'a> {
    pub app: &'a Arc<AppState>,
    /// The bot's own project, or the project of the item the owner edits.
    pub project_id: &'a str,
    pub actor: Actor<'a>,
}

/// A bot of the project, by id or name.
pub fn project_bot(app: &AppState, project_id: &str, name_or_id: &str) -> anyhow::Result<String> {
    let bots = app.db.list_bots(Some(project_id))?;
    bots.iter()
        .find(|b| b.id == name_or_id)
        .or_else(|| {
            bots.iter()
                .find(|b| b.name.eq_ignore_ascii_case(name_or_id))
        })
        .map(|b| b.id.clone())
        .ok_or_else(|| anyhow::anyhow!("no bot '{name_or_id}' in this project"))
}

/// Why a draft or a discarded item refuses an edit that needs a live one.
pub fn not_live(item: &Item) -> Option<Unmet> {
    match item.state {
        ItemState::Live => None,
        ItemState::Draft => Some(unmet(
            "draft",
            format!("{} is a draft. Finalize the draft first.", item.id),
            Some("The owner finalizes it into a column."),
        )),
        ItemState::Discarded => Some(unmet(
            "discarded",
            format!("{} was discarded.", item.id),
            None,
        )),
    }
}

impl Editor<'_> {
    /// An item of this project that the caller may see: a draft only by the
    /// owner and the bots invited to it. Others are told it doesn't exist.
    pub fn own_item(&self, id: &str) -> anyhow::Result<()> {
        let visible = self.app.db.board_read(|t| {
            if t.item_project(id)?.as_deref() != Some(self.project_id) {
                return Ok(false);
            }
            super::drafts::visible_in(t, id, &self.actor)
        })?;
        anyhow::ensure!(visible, "no item {id} in this project");
        Ok(())
    }

    /// Run a write holding the feed, and push the item it changed once it
    /// has committed. `None` for the item pushes nothing.
    pub(super) fn published<T>(
        &self,
        kind: ChangeKind,
        from_column: Option<String>,
        write: impl FnOnce() -> anyhow::Result<(T, Option<String>)>,
    ) -> anyhow::Result<T> {
        let mut feed = self.app.board.writer();
        let (out, item_id) = write()?;
        if let Some(item_id) = item_id {
            let card = card_after_commit(&self.app.db, &item_id);
            feed.publish(Change {
                project_id: self.project_id,
                kind,
                item_id: &item_id,
                card,
                from_column,
            });
        }
        Ok(out)
    }

    /// Load, check the version, guard and write, in one transaction. Only an
    /// edit that `on_draft` allows reaches a draft.
    pub(super) fn guarded(
        &self,
        id: &str,
        expected: u64,
        on_draft: bool,
        guard: impl FnOnce(&Item, &Who) -> Vec<Unmet>,
        write: impl FnOnce(&BoardTx<'_>) -> anyhow::Result<Write<Item>>,
    ) -> anyhow::Result<Edited> {
        self.own_item(id)?;
        self.published(ChangeKind::ItemUpserted, None, || {
            let edited = self.app.db.board_tx(|t| {
                let (item, _, who) = load_in(t, id, &self.actor)?;
                if item.version != expected {
                    return Ok(Edited::Conflict(Box::new(item)));
                }
                let refusal = if on_draft && item.state == ItemState::Draft {
                    None
                } else {
                    not_live(&item)
                };
                if let Some(refusal) = refusal {
                    return Ok(Edited::Refused(vec![refusal]));
                }
                let unmet = guard(&item, &who);
                if !unmet.is_empty() {
                    return Ok(Edited::Refused(unmet));
                }
                Ok(match write(t)? {
                    Write::Done(item) => Edited::Done(Box::new(item)),
                    Write::Conflict(item) => Edited::Conflict(item),
                })
            })?;
            let changed = matches!(edited, Edited::Done(_)).then(|| id.to_string());
            Ok((edited, changed))
        })
    }

    /// A new item in the board's inbox, or a draft (the owner's alone).
    pub fn create(&self, req: &c::ItemCreate) -> anyhow::Result<Edited> {
        let mut unmet = guards::check_new(&req.title);
        unmet.extend(guards::check_type(req.r#type));
        if !unmet.is_empty() {
            return Ok(Edited::Refused(unmet));
        }
        let draft = req.draft == Some(true);
        if draft && matches!(self.actor, Actor::Bot { .. }) {
            return Err(invalid(
                "only the owner starts a draft; create the item instead",
            ));
        }
        if let Some(parent) = req.parent_id.as_deref().filter(|p| !p.is_empty()) {
            self.own_item(parent)?;
        }
        let labels = crate::compat018::create_labels(req);
        let new = NewItem {
            project_id: self.project_id,
            board_id: req.board_id.as_deref().filter(|b| !b.is_empty()),
            item_type: ItemType::from_wire(req.r#type)?,
            title: req.title.trim(),
            description: req.description.as_deref().unwrap_or_default(),
            size: req.size.map(Size::from_wire).transpose()?,
            priority: req
                .priority
                .map(Priority::from_wire)
                .transpose()?
                .unwrap_or(Priority::P2),
            labels: &labels,
            parent_id: req.parent_id.as_deref().filter(|p| !p.is_empty()),
            acceptance_criteria: &req.acceptance_criteria,
        };
        self.published(ChangeKind::ItemUpserted, None, || {
            let item = self.app.db.board_tx(|t| match draft {
                true => t.create_draft(&new, &self.actor),
                false => t.create_item(&new, &self.actor),
            })?;
            let id = item.id.clone();
            Ok((Edited::Done(Box::new(item)), Some(id)))
        })
    }

    pub fn update(&self, req: &c::ItemUpdate) -> anyhow::Result<Edited> {
        let retired = req.r#type.map(guards::check_type).unwrap_or_default();
        if !retired.is_empty() {
            return Ok(Edited::Refused(retired));
        }
        if let Some(parent) = req.parent_id.as_deref().filter(|p| !p.is_empty()) {
            self.own_item(parent)?;
        }
        let group = crate::compat018::update_group(req);
        let priority = req.priority.map(Priority::from_wire).transpose()?;
        let criteria: Option<Vec<String>> = req
            .acceptance_criteria
            .as_ref()
            .map(|l| l.values.iter().map(|t| t.trim().to_string()).collect());
        if let Some(criteria) = &criteria {
            anyhow::ensure!(
                !criteria.iter().any(String::is_empty),
                "an acceptance criterion can't be blank"
            );
            let distinct: std::collections::HashSet<&String> = criteria.iter().collect();
            // A check is kept by text, so two equal texts would share one.
            anyhow::ensure!(
                distinct.len() == criteria.len(),
                "two acceptance criteria have the same text"
            );
        }
        let edit = ItemEdit {
            title: req.title.as_deref(),
            description: req.description.as_deref(),
            size: req.size.map(Size::from_wire).transpose()?.map(Some),
            priority,
            labels: req.labels.as_ref().map(|l| l.values.as_slice()),
            group: group.as_ref().map(|(g, values)| (*g, values.as_slice())),
            acceptance_criteria: criteria.as_deref(),
            // An empty parent clears it.
            parent_id: req
                .parent_id
                .as_deref()
                .map(|p| (!p.is_empty()).then_some(p)),
            item_type: req.r#type.map(ItemType::from_wire).transpose()?,
        };
        let retype = edit.item_type;
        let guard = |item: &Item, who: &Who| {
            // A draft is the owner's to shape with the bots they invited.
            if item.state == ItemState::Draft {
                return Vec::new();
            }
            // Raising to P0 is the lead's or the owner's call.
            let to_p0 = priority == Some(Priority::P0) && item.priority != Priority::P0;
            // So is changing an item's type, e.g. task → spike.
            let retyped = retype.is_some_and(|t| t != item.item_type);
            let mut unmet = if to_p0 || retyped {
                guards::check_lead(who)
            } else {
                Vec::new()
            };
            if criteria.is_some() {
                unmet.extend(guards::check_ac_edit(item));
            }
            unmet
        };
        self.guarded(&req.id, req.expected_version, true, guard, |t| {
            t.update_item(&req.id, req.expected_version, &edit, &self.actor)
        })
    }

    pub fn link(&self, req: &c::ItemAddLink) -> anyhow::Result<ItemLink> {
        self.own_item(&req.id)?;
        self.refuse_discarded(&req.id)?;
        let kind = LinkKind::from_wire(req.kind)?;
        if matches!(
            kind,
            LinkKind::ItemBlocks | LinkKind::ItemRelates | LinkKind::ItemDuplicates
        ) {
            self.own_item(&req.r#ref)?;
            anyhow::ensure!(req.r#ref != req.id, "an item can't link to itself");
        }
        self.published(ChangeKind::ItemUpserted, None, || {
            let label = req.label.as_deref();
            let link = self
                .app
                .db
                .add_item_link(&req.id, kind, &req.r#ref, label, &self.actor)?;
            Ok((link, Some(req.id.clone())))
        })
    }

    pub fn unlink(&self, req: &c::ItemRemoveLink) -> anyhow::Result<bool> {
        self.own_item(&req.id)?;
        self.refuse_discarded(&req.id)?;
        let kind = LinkKind::from_wire(req.kind)?;
        self.published(ChangeKind::ItemUpserted, None, || {
            let removed = self
                .app
                .db
                .remove_item_link(&req.id, kind, &req.r#ref, &self.actor)?;
            Ok((removed, removed.then(|| req.id.clone())))
        })
    }

    /// A discarded draft keeps its history and takes no change.
    pub(super) fn refuse_discarded(&self, id: &str) -> anyhow::Result<()> {
        let state = self.app.db.board_read(|t| t.item_state(id))?;
        anyhow::ensure!(
            state != Some(ItemState::Discarded),
            invalid(format!("{id} was discarded"))
        );
        Ok(())
    }
}

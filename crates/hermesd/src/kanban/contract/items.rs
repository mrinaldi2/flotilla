//! Items and cards: model → contract (infallible) and contract → model (checked).

use bus::contract::board as c;

use super::super::model as m;
use super::{all, at, maybe_at, stamp, MapError};

impl From<m::Item> for c::Item {
    fn from(v: m::Item) -> Self {
        let mut item = c::Item {
            id: v.id,
            board_id: v.board_id,
            seq: v.seq,
            r#type: v.item_type.into(),
            title: v.title,
            description: v.description,
            size: v.size.map(Into::into),
            priority: v.priority.into(),
            rank: v.rank,
            column_key: v.column_key,
            category: v.category.into(),
            blocked: v.blocked.map(Into::into),
            assignee: v.assignee,
            parent_id: v.parent_id,
            release_id: v.release_id,
            labels: v.labels,
            acceptance_criteria: v.acceptance_criteria.into_iter().map(Into::into).collect(),
            people: v.people.into_iter().map(Into::into).collect(),
            verifications: v.verifications.into_iter().map(Into::into).collect(),
            created_by: v.created_by,
            created_at: stamp(v.created_at),
            updated_at: stamp(v.updated_at),
            state_entered_at: stamp(v.state_entered_at),
            done_at: v.done_at.and_then(stamp),
            version: v.version,
            state: v.state.as_str().to_string(),
            ..Default::default()
        };
        crate::compat018::item_out(&mut item);
        item
    }
}

impl TryFrom<c::Item> for m::Item {
    type Error = MapError;

    fn try_from(mut v: c::Item) -> Result<Self, MapError> {
        crate::compat018::item_in(&mut v);
        Ok(m::Item {
            id: v.id,
            board_id: v.board_id,
            seq: v.seq,
            item_type: m::ItemType::from_wire(v.r#type)?,
            title: v.title,
            description: v.description,
            size: v.size.map(m::Size::from_wire).transpose()?,
            priority: m::Priority::from_wire(v.priority)?,
            rank: v.rank,
            column_key: v.column_key,
            category: m::ColumnCategory::from_wire(v.category)?,
            blocked: v.blocked.map(TryInto::try_into).transpose()?,
            assignee: v.assignee,
            parent_id: v.parent_id,
            release_id: v.release_id,
            labels: v.labels,
            acceptance_criteria: all(v.acceptance_criteria)?,
            people: all(v.people)?,
            verifications: all(v.verifications)?,
            created_by: v.created_by,
            created_at: at(v.created_at, "Item.created_at")?,
            updated_at: at(v.updated_at, "Item.updated_at")?,
            state_entered_at: at(v.state_entered_at, "Item.state_entered_at")?,
            done_at: maybe_at(v.done_at, "Item.done_at")?,
            version: v.version,
            // A home older than 0.19 sends none: every item there is live.
            state: match v.state.as_str() {
                "" => m::ItemState::Live,
                text => m::ItemState::parse(text).ok_or(MapError::UnknownEnum("ItemState"))?,
            },
        })
    }
}

impl From<m::ItemCard> for c::ItemCard {
    fn from(v: m::ItemCard) -> Self {
        let mut card = c::ItemCard {
            id: v.id,
            board_id: v.board_id,
            r#type: v.item_type.into(),
            title: v.title,
            priority: v.priority.into(),
            size: v.size.map(Into::into),
            rank: v.rank,
            column_key: v.column_key,
            assignee: v.assignee,
            labels: v.labels,
            blocked: v.blocked,
            stale: v.stale,
            ac_checked: v.ac_checked,
            ac_total: v.ac_total,
            version: v.version,
            owner_commented_at: v.owner_commented_at.and_then(stamp),
            draft: v.draft,
            invited: v.invited,
            ..Default::default()
        };
        crate::compat018::card_out(&mut card);
        card
    }
}

impl TryFrom<c::ItemCard> for m::ItemCard {
    type Error = MapError;

    fn try_from(mut v: c::ItemCard) -> Result<Self, MapError> {
        crate::compat018::card_in(&mut v);
        Ok(m::ItemCard {
            id: v.id,
            board_id: v.board_id,
            item_type: m::ItemType::from_wire(v.r#type)?,
            title: v.title,
            priority: m::Priority::from_wire(v.priority)?,
            size: v.size.map(m::Size::from_wire).transpose()?,
            rank: v.rank,
            column_key: v.column_key,
            assignee: v.assignee,
            labels: v.labels,
            blocked: v.blocked,
            stale: v.stale,
            ac_checked: v.ac_checked,
            ac_total: v.ac_total,
            version: v.version,
            owner_commented_at: maybe_at(v.owner_commented_at, "owner_commented_at")?,
            draft: v.draft,
            invited: v.invited,
        })
    }
}

impl From<m::Unmet> for c::Unmet {
    fn from(v: m::Unmet) -> Self {
        c::Unmet {
            code: v.code,
            text: v.text,
            fix: v.fix,
        }
    }
}

impl TryFrom<c::Unmet> for m::Unmet {
    type Error = MapError;

    fn try_from(v: c::Unmet) -> Result<Self, MapError> {
        Ok(m::Unmet {
            code: v.code,
            text: v.text,
            fix: v.fix,
        })
    }
}

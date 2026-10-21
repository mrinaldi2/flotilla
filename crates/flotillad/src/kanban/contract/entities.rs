//! Messages: model → contract (infallible) and contract → model (checked).

use bus::contract::board as c;
use bus::contract::pbjson_types::Struct;

use super::super::flow::{tokens, ColumnPolicy};
use super::super::labels::LabelGroup;
use super::super::model as m;
use super::{at, maybe_at, stamp, MapError};

impl From<m::BoardSettings> for c::BoardSettings {
    fn from(v: m::BoardSettings) -> Self {
        c::BoardSettings {
            project_id: v.project_id,
            key: v.key,
            next_seq: v.next_seq,
            stale_after_hours: v.stale_after_hours,
            required_machines: crate::compat018::machines_out(&v.required_machines),
            required_machines_by_label: v
                .required_machines
                .into_iter()
                .map(|(label, values)| (label, c::StringList { values }))
                .collect(),
            home_daemon_id: v.home_daemon_id,
            version: v.version,
        }
    }
}

impl TryFrom<c::BoardSettings> for m::BoardSettings {
    type Error = MapError;

    fn try_from(v: c::BoardSettings) -> Result<Self, MapError> {
        let required_machines = crate::compat018::machines_in(&v);
        Ok(m::BoardSettings {
            project_id: v.project_id,
            key: v.key,
            next_seq: v.next_seq,
            stale_after_hours: v.stale_after_hours,
            required_machines,
            home_daemon_id: v.home_daemon_id,
            version: v.version,
        })
    }
}

impl From<m::Board> for c::Board {
    fn from(v: m::Board) -> Self {
        c::Board {
            id: v.id,
            project_id: v.project_id,
            name: v.name,
            key: v.key,
            description: v.description,
            ord: v.ord,
            is_default: v.is_default,
            structure_policy: v.structure_policy.as_str().to_string(),
            flow: v.flow.as_str().to_string(),
            automations: v.automations.into_iter().collect(),
            archived: v.archived_at.is_some(),
            version: v.version,
            label_groups: v.label_groups.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<LabelGroup> for c::LabelGroup {
    fn from(v: LabelGroup) -> Self {
        c::LabelGroup {
            name: v.name,
            values: v.values,
        }
    }
}

impl From<c::LabelGroup> for LabelGroup {
    fn from(v: c::LabelGroup) -> Self {
        LabelGroup {
            name: v.name,
            values: v.values,
        }
    }
}

impl From<m::BoardColumn> for c::BoardColumn {
    fn from(v: m::BoardColumn) -> Self {
        c::BoardColumn {
            project_id: v.project_id,
            board_id: v.board_id,
            key: v.key,
            name: v.name,
            ord: v.ord,
            category: v.category.into(),
            kind: v.kind.into(),
            wip_limit: v.wip_limit,
            wip_scope: v.wip_scope.into(),
            visible: v.visible,
            movers: v.policy.movers.iter().map(|m| m.token()).collect(),
            rules: v.policy.rules.iter().map(|r| r.token()).collect(),
            allowed_from: v.policy.allowed_from,
            on_entry: v.on_entry.map_or(c::Role::Unspecified as i32, |r| r.into()),
        }
    }
}

impl TryFrom<c::BoardColumn> for m::BoardColumn {
    type Error = MapError;

    fn try_from(v: c::BoardColumn) -> Result<Self, MapError> {
        let category = m::ColumnCategory::from_wire(v.category)?;
        // A home older than 0.19 sends no kind: it follows the category.
        let kind = match v.kind {
            0 => category.kind(),
            kind => m::ColumnKind::from_wire(kind)?,
        };
        Ok(m::BoardColumn {
            project_id: v.project_id,
            board_id: v.board_id,
            key: v.key,
            name: v.name,
            ord: v.ord,
            category,
            kind,
            wip_limit: v.wip_limit,
            wip_scope: m::WipScope::from_wire(v.wip_scope)?,
            visible: v.visible,
            policy: ColumnPolicy {
                movers: tokens::movers(&v.movers).map_err(|_| MapError::UnknownEnum("Mover"))?,
                rules: tokens::rules(&v.rules).map_err(|_| MapError::UnknownEnum("EntryRule"))?,
                allowed_from: v.allowed_from,
            },
            // A mirror's columns: the home assigns on entry, never a mirror.
            on_entry: match v.on_entry {
                0 => None,
                role => m::Role::from_wire(role).ok(),
            },
        })
    }
}

impl From<m::ProjectRole> for c::ProjectRole {
    fn from(v: m::ProjectRole) -> Self {
        c::ProjectRole {
            project_id: v.project_id,
            role: v.role.into(),
            bot_id: v.bot_id,
            machine: v.machine,
        }
    }
}

impl TryFrom<c::ProjectRole> for m::ProjectRole {
    type Error = MapError;

    fn try_from(v: c::ProjectRole) -> Result<Self, MapError> {
        Ok(m::ProjectRole {
            project_id: v.project_id,
            role: m::Role::from_wire(v.role)?,
            bot_id: v.bot_id,
            machine: v.machine,
        })
    }
}

impl From<m::Blocked> for c::Blocked {
    fn from(v: m::Blocked) -> Self {
        c::Blocked {
            by: v.by,
            reason: v.reason,
            since: stamp(v.since),
        }
    }
}

impl TryFrom<c::Blocked> for m::Blocked {
    type Error = MapError;

    fn try_from(v: c::Blocked) -> Result<Self, MapError> {
        Ok(m::Blocked {
            by: v.by,
            reason: v.reason,
            since: at(v.since, "Blocked.since")?,
        })
    }
}

impl From<m::AcceptanceCriterion> for c::AcceptanceCriterion {
    fn from(v: m::AcceptanceCriterion) -> Self {
        c::AcceptanceCriterion {
            idx: v.idx,
            text: v.text,
            checked: v.checked,
            checked_by: v.checked_by,
            checked_at: v.checked_at.and_then(stamp),
            machine: v.machine,
            post_install: v.post_install,
        }
    }
}

impl TryFrom<c::AcceptanceCriterion> for m::AcceptanceCriterion {
    type Error = MapError;

    fn try_from(v: c::AcceptanceCriterion) -> Result<Self, MapError> {
        Ok(m::AcceptanceCriterion {
            idx: v.idx,
            text: v.text,
            checked: v.checked,
            checked_by: v.checked_by,
            checked_at: maybe_at(v.checked_at, "AcceptanceCriterion.checked_at")?,
            machine: v.machine,
            post_install: v.post_install,
        })
    }
}

impl From<m::ItemPerson> for c::ItemPerson {
    fn from(v: m::ItemPerson) -> Self {
        c::ItemPerson {
            bot_id: v.bot_id,
            role: v.role.into(),
        }
    }
}

impl TryFrom<c::ItemPerson> for m::ItemPerson {
    type Error = MapError;

    fn try_from(v: c::ItemPerson) -> Result<Self, MapError> {
        Ok(m::ItemPerson {
            bot_id: v.bot_id,
            role: m::PersonRole::from_wire(v.role)?,
        })
    }
}

impl From<m::ItemVerification> for c::ItemVerification {
    fn from(v: m::ItemVerification) -> Self {
        c::ItemVerification {
            machine: v.machine,
            result: v.result.into(),
            by: v.by,
            at: stamp(v.at),
            note: v.note,
        }
    }
}

impl TryFrom<c::ItemVerification> for m::ItemVerification {
    type Error = MapError;

    fn try_from(v: c::ItemVerification) -> Result<Self, MapError> {
        Ok(m::ItemVerification {
            machine: v.machine,
            result: m::VerificationResult::from_wire(v.result)?,
            by: v.by,
            at: at(v.at, "ItemVerification.at")?,
            note: v.note,
        })
    }
}

impl From<m::ItemLink> for c::ItemLink {
    fn from(v: m::ItemLink) -> Self {
        c::ItemLink {
            item_id: v.item_id,
            kind: v.kind.into(),
            r#ref: v.target,
            label: v.label,
            created_by: v.created_by,
            at: stamp(v.at),
        }
    }
}

impl TryFrom<c::ItemLink> for m::ItemLink {
    type Error = MapError;

    fn try_from(v: c::ItemLink) -> Result<Self, MapError> {
        Ok(m::ItemLink {
            item_id: v.item_id,
            kind: m::LinkKind::from_wire(v.kind)?,
            target: v.r#ref,
            label: v.label,
            created_by: v.created_by,
            at: at(v.at, "ItemLink.at")?,
        })
    }
}

impl From<m::ItemComment> for c::ItemComment {
    fn from(v: m::ItemComment) -> Self {
        c::ItemComment {
            id: v.id,
            item_id: v.item_id,
            author: v.author,
            body: v.body,
            reply_to: v.reply_to,
            at: stamp(v.at),
        }
    }
}

impl TryFrom<c::ItemComment> for m::ItemComment {
    type Error = MapError;

    fn try_from(v: c::ItemComment) -> Result<Self, MapError> {
        Ok(m::ItemComment {
            id: v.id,
            item_id: v.item_id,
            author: v.author,
            body: v.body,
            reply_to: v.reply_to,
            at: at(v.at, "ItemComment.at")?,
        })
    }
}

impl From<m::ItemEvent> for c::ItemEvent {
    fn from(v: m::ItemEvent) -> Self {
        c::ItemEvent {
            id: v.id,
            item_id: v.item_id,
            at: stamp(v.at),
            actor: v.actor,
            kind: v.kind.into(),
            from: v.from,
            to: v.to,
            field: v.field,
            note: v.note,
        }
    }
}

impl TryFrom<c::ItemEvent> for m::ItemEvent {
    type Error = MapError;

    fn try_from(v: c::ItemEvent) -> Result<Self, MapError> {
        Ok(m::ItemEvent {
            id: v.id,
            item_id: v.item_id,
            at: at(v.at, "ItemEvent.at")?,
            actor: v.actor,
            kind: m::ItemEventKind::from_wire(v.kind)?,
            from: v.from,
            to: v.to,
            field: v.field,
            note: v.note,
        })
    }
}

impl From<m::Template> for c::Template {
    fn from(v: m::Template) -> Self {
        c::Template {
            project_id: v.project_id,
            kind: v.kind.into(),
            name: v.name,
            version: v.version,
            // A JSON object is exactly what a Struct holds; anything else
            // (never stored by the board) travels as no body.
            body: serde_json::from_value::<Struct>(v.body).ok(),
        }
    }
}

impl TryFrom<c::Template> for m::Template {
    type Error = MapError;

    fn try_from(v: c::Template) -> Result<Self, MapError> {
        Ok(m::Template {
            project_id: v.project_id,
            kind: m::TemplateKind::from_wire(v.kind)?,
            name: v.name,
            version: v.version,
            body: v
                .body
                .map(|body| {
                    serde_json::to_value(body).map_err(|_| MapError::OutOfRange("Template.body"))
                })
                .transpose()?
                .unwrap_or_else(|| serde_json::json!({})),
        })
    }
}

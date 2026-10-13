//! Undoing one change to a draft (0.19.0 K5): the owner's review lists a
//! bot's changes since they last looked, each with its own Undo.
//!
//! - A field goes back to the revision's `from`, through the same edit the
//!   owner makes from the drawer.
//! - An attachment the change added is detached; one it detached is
//!   attached again. A link it added is removed; one it removed comes back.
//!
//! The undo is a revision of its own, by the owner, so the history keeps
//! both. Only a draft takes it: once live, an item is edited as usual.

use std::sync::Arc;

use bus::contract::board as c;

use super::edit::{Edited, Editor};
use super::model::{DraftRevision, Item, ItemState, ItemType, LinkKind, Priority, Size, Unmet};
use crate::actor::Actor;
use crate::app::AppState;
use crate::db::ItemEdit;
use crate::decisions::{invalid, not_found};
use crate::kanban::guards::unmet;

/// Take back revision `req.revision` of the draft `req.id`.
pub fn undo(
    app: &Arc<AppState>,
    actor: &Actor<'_>,
    req: &c::ItemDraftUndo,
) -> anyhow::Result<Edited> {
    let project_id = app
        .db
        .board_read(|t| t.item_project(&req.id))?
        .ok_or_else(|| not_found(format!("no item {}", req.id)))?;
    let revision = app
        .db
        .board_read(|t| t.draft_revisions(&req.id, Some(req.revision - 1)))?
        .into_iter()
        .next()
        .filter(|r| r.id == req.revision)
        .ok_or_else(|| not_found(format!("no change {} on {}", req.revision, req.id)))?;
    let editor = Editor {
        app,
        project_id: &project_id,
        actor: *actor,
    };
    match revision.field.as_str() {
        "attachment" => undo_attachment(&editor, req, &revision),
        "link" => undo_link(&editor, req, &revision),
        field => undo_field(&editor, req, field, revision.from.as_deref().unwrap_or("")),
    }
}

/// Why an undo needs a draft, or nothing when it is one.
fn not_draft(item: &Item) -> Vec<Unmet> {
    if item.state == ItemState::Draft {
        return Vec::new();
    }
    vec![unmet(
        "draft.live",
        format!("{} is not a draft any more; edit it instead.", item.id),
        None,
    )]
}

fn undo_field(
    editor: &Editor<'_>,
    req: &c::ItemDraftUndo,
    field: &str,
    from: &str,
) -> anyhow::Result<Edited> {
    let unknown = || invalid(format!("{field} can't go back to “{from}”"));
    let list: Vec<String>;
    let mut edit = ItemEdit::default();
    match field {
        "title" => edit.title = Some(from),
        "description" => edit.description = Some(from),
        "type" => edit.item_type = Some(ItemType::parse(from).ok_or_else(unknown)?),
        "size" if from.is_empty() => edit.size = Some(None),
        "size" => edit.size = Some(Some(Size::parse(from).ok_or_else(unknown)?)),
        "priority" => edit.priority = Some(Priority::parse(from).ok_or_else(unknown)?),
        "labels" => {
            list = split(from, ",");
            edit.labels = Some(&list);
        }
        "acceptance_criteria" => {
            list = split(from, "\n");
            edit.acceptance_criteria = Some(&list);
        }
        "parent" => edit.parent_id = Some((!from.is_empty()).then_some(from)),
        other => return Err(invalid(format!("a change to {other} can't be undone"))),
    }
    editor.guarded(
        &req.id,
        req.expected_version,
        true,
        |item, _| not_draft(item),
        |t| t.update_item(&req.id, req.expected_version, &edit, &editor.actor),
    )
}

/// A revision's list, as `board_drafts::revised` joined it.
fn split(text: &str, by: &str) -> Vec<String> {
    text.split(by)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// The draft as it is, when the caller's version is current and it is
/// still a draft; else how the undo came out.
fn current(editor: &Editor<'_>, req: &c::ItemDraftUndo) -> anyhow::Result<Result<Item, Edited>> {
    editor.own_item(&req.id)?;
    let item = editor
        .app
        .db
        .get_item(&req.id)?
        .ok_or_else(|| not_found(format!("no item {}", req.id)))?;
    if item.version != req.expected_version {
        return Ok(Err(Edited::Conflict(Box::new(item))));
    }
    let refusal = not_draft(&item);
    if !refusal.is_empty() {
        return Ok(Err(Edited::Refused(refusal)));
    }
    Ok(Ok(item))
}

fn undo_attachment(
    editor: &Editor<'_>,
    req: &c::ItemDraftUndo,
    revision: &DraftRevision,
) -> anyhow::Result<Edited> {
    if let Err(outcome) = current(editor, req)? {
        return Ok(outcome);
    }
    let attachments = editor.app.db.board_read(|t| t.attachments(&req.id))?;
    // The newest attachment of that name in the state the change left it.
    let (name, removed) = match (&revision.from, &revision.to) {
        (None, Some(added)) => (added, false),
        (Some(detached), None) => (detached, true),
        _ => return Err(invalid("that change can't be undone")),
    };
    let target = attachments
        .iter()
        .rev()
        .find(|a| &a.name == name && a.removed == removed)
        .ok_or_else(|| invalid(format!("{name} was changed again since; nothing to undo")))?;
    let attachment_id = target.id.clone();
    if removed {
        editor.published(super::feed::ChangeKind::ItemUpserted, None, || {
            let done = editor
                .app
                .db
                .board_tx(|t| t.reattach(&req.id, &attachment_id, &editor.actor))?;
            Ok((done, done.then(|| req.id.clone())))
        })?;
    } else {
        editor.detach(&c::ItemDetach {
            id: req.id.clone(),
            attachment_id,
        })?;
    }
    after(editor, &req.id)
}

fn undo_link(
    editor: &Editor<'_>,
    req: &c::ItemDraftUndo,
    revision: &DraftRevision,
) -> anyhow::Result<Edited> {
    if let Err(outcome) = current(editor, req)? {
        return Ok(outcome);
    }
    let (what, added) = match (&revision.from, &revision.to) {
        (None, Some(added)) => (added, true),
        (Some(removed), None) => (removed, false),
        _ => return Err(invalid("that change can't be undone")),
    };
    let (kind, target) = link_of(what).ok_or_else(|| invalid(format!("no link in “{what}”")))?;
    if added {
        editor.unlink(&c::ItemRemoveLink {
            id: req.id.clone(),
            kind: i32::from(kind),
            r#ref: target.to_string(),
        })?;
    } else {
        editor.link(&c::ItemAddLink {
            id: req.id.clone(),
            kind: i32::from(kind),
            r#ref: target.to_string(),
            label: None,
        })?;
    }
    after(editor, &req.id)
}

/// "item:blocks:RB-3" as `board_notes` records it: the longest kind that
/// prefixes it, and the rest.
fn link_of(what: &str) -> Option<(LinkKind, &str)> {
    LinkKind::ALL
        .iter()
        .filter_map(|kind| {
            what.strip_prefix(kind.as_str())
                .and_then(|rest| rest.strip_prefix(':'))
                .map(|rest| (*kind, rest))
        })
        .max_by_key(|(kind, _)| kind.as_str().len())
}

fn after(editor: &Editor<'_>, id: &str) -> anyhow::Result<Edited> {
    let item = editor
        .app
        .db
        .get_item(id)?
        .ok_or_else(|| not_found(format!("no item {id}")))?;
    Ok(Edited::Done(Box::new(item)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_revision_splits_on_its_longest_kind() {
        assert_eq!(
            link_of("item:blocks:RB-3"),
            Some((LinkKind::ItemBlocks, "RB-3"))
        );
        assert_eq!(
            link_of("artifact:specs/a.md"),
            Some((LinkKind::Artifact, "specs/a.md"))
        );
        assert_eq!(link_of("nothing"), None);
    }

    #[test]
    fn a_revision_list_splits_back_into_its_values() {
        assert_eq!(split("ui, web", ","), ["ui", "web"]);
        assert_eq!(split("", "\n"), Vec::<String>::new());
        assert_eq!(split("One\nTwo", "\n"), ["One", "Two"]);
    }
}

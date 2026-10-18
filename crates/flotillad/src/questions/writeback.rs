//! An answer written back into the research it answers (0.22.0): under
//! "Answers from the owner" in the research document of the question's card,
//! or of its node's spikes. A file attached to the card gets a new revision
//! (the old one stays in the card's history); a linked artifact, a shared
//! file, is edited in place.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bus::contract::board as c;

use crate::actor::Actor;
use crate::app::AppState;
use crate::kanban::attach::{artifacts_of, root, Source};
use crate::kanban::edit::Editor;
use crate::kanban::model::{AttachmentKind, ItemAttachment, LinkKind};

const HEADING: &str = "## Answers from the owner";

/// A card's research document.
enum Doc {
    /// A file attached to the card.
    Attached(Box<ItemAttachment>),
    /// An artifact, linked or attached as a doc: its path in artifacts.
    Artifact(String),
}

fn research(s: &str) -> bool {
    s.to_lowercase().contains("research")
}

/// The card's newest research document.
fn research_doc(app: &AppState, item_id: &str) -> anyhow::Result<Option<Doc>> {
    let (attachments, links) = app
        .db
        .board_read(|t| Ok((t.attachments(item_id)?, t.item_links(item_id)?)))?;
    let attached = attachments
        .into_iter()
        .filter(|a| !a.removed && !a.path.is_empty() && (research(&a.name) || research(&a.path)))
        .max_by_key(|a| a.at);
    if let Some(a) = attached {
        return Ok(Some(match a.kind {
            AttachmentKind::Doc => Doc::Artifact(a.path),
            _ => Doc::Attached(Box::new(a)),
        }));
    }
    Ok(links
        .into_iter()
        .filter(|l| {
            l.kind == LinkKind::Artifact
                && (research(&l.target) || l.label.as_deref().is_some_and(research))
        })
        .max_by_key(|l| l.at)
        .map(|l| Doc::Artifact(l.target)))
}

/// The answer, as the document gets it.
fn section(existing: &str, question: &c::Question) -> String {
    let answer = question.answer.as_ref();
    let option = answer
        .and_then(|a| a.option.as_deref())
        .and_then(|key| question.options.iter().find(|o| o.key == key))
        .map(|o| format!(" (picked: {})", o.label))
        .unwrap_or_default();
    let text = answer.map(|a| a.text.as_str()).unwrap_or_default();
    let mut out = String::new();
    if !existing.ends_with('\n') && !existing.is_empty() {
        out.push('\n');
    }
    if !existing.contains(HEADING) {
        out.push_str(&format!("\n{HEADING}\n"));
    }
    out.push_str(&format!("\n**{}**\n\n{text}{option}\n", question.text));
    out
}

/// Inside the project's artifacts, links resolved.
fn confined(artifacts: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let base = artifacts.canonicalize()?;
    let path = base.join(rel).canonicalize()?;
    anyhow::ensure!(
        path.starts_with(&base),
        "{rel} is outside the project's artifacts"
    );
    anyhow::ensure!(path.is_file(), "{rel} is not a file");
    Ok(path)
}

/// Write the answer of question `id` into its research, if it has one.
pub(super) fn write(app: &Arc<AppState>, id: &str) -> anyhow::Result<()> {
    let Some(question) = super::wire::load(app, id)? else {
        return Ok(());
    };
    if question.answer.is_none() {
        return Ok(());
    }
    let cards: Vec<String> = match (&question.item_id, &question.plan_node_id) {
        (Some(item), _) => vec![item.clone()],
        (None, Some(node)) => app
            .db
            .board_read(|t| t.plan_spikes(node))?
            .into_iter()
            .map(|s| s.id)
            .collect(),
        (None, None) => Vec::new(),
    };
    let artifacts = artifacts_of(app, &question.project_id)?;
    for card in cards {
        let Some(doc) = research_doc(app, &card)? else {
            continue;
        };
        let written = match doc {
            Doc::Artifact(rel) => {
                let path = confined(&artifacts, &rel)?;
                let existing = fs::read_to_string(&path)?;
                fs::write(&path, existing.clone() + &section(&existing, &question))?;
                rel
            }
            Doc::Attached(old) => revise(app, &artifacts, &card, &question, *old)?,
        };
        app.db.set_question_written(id, &written)?;
        return Ok(());
    }
    Ok(())
}

/// A new revision of the attached file with the answer; the old one is
/// detached and stays in the card's history.
fn revise(
    app: &Arc<AppState>,
    artifacts: &Path,
    card: &str,
    question: &c::Question,
    old: ItemAttachment,
) -> anyhow::Result<String> {
    let existing = fs::read_to_string(confined(artifacts, &old.path)?)?;
    let staging = artifacts.join("items").join(card).join(".uploads");
    fs::create_dir_all(&staging)?;
    let staged = staging.join(format!("answer-{}.md", question.id));
    fs::write(&staged, existing.clone() + &section(&existing, question))?;
    let ed = Editor {
        app,
        project_id: &question.project_id,
        actor: Actor::Daemon,
    };
    let attached = ed.attach(
        &c::ItemAttach {
            id: card.to_string(),
            caption: (!old.caption.is_empty()).then(|| old.caption.clone()),
            name: Some(old.name.clone()),
            ..Default::default()
        },
        Source::File {
            path: staged.clone(),
            roots: root(&staging),
        },
    );
    fs::remove_file(&staged).ok();
    let attached = attached?;
    ed.detach(&c::ItemDetach {
        id: card.to_string(),
        attachment_id: old.id,
    })?;
    Ok(attached.path)
}

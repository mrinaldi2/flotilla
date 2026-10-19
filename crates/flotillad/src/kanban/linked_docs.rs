//! A card's documents (0.22.0, `artifacts_v2`): its attachments, and each
//! artifact it links with an `artifact` link, shown as a document that opens
//! in place, so a card has one list of documents instead of two.

use std::collections::HashSet;
use std::path::Path;

use bus::contract::board as c;

use super::attachments::to_wire;
use super::contract::stamp;
use super::model::{ItemAttachment, ItemLink, LinkKind};
use crate::chat::artifact_groups::{rel_in, roots};
use crate::db::BoardTx;

/// The id a linked artifact carries as a document: `link:` and the link's
/// target, so a client removes it as a link, not with `item_detach`.
pub const LINK_ID_PREFIX: &str = "link:";

/// An item's attachments, detached ones included, then its linked artifacts.
pub fn documents(
    t: &BoardTx<'_>,
    item_id: &str,
    artifacts: &Path,
) -> anyhow::Result<Vec<c::ItemAttachment>> {
    Ok(with_links(
        t.attachments(item_id)?,
        &t.item_links(item_id)?,
        artifacts,
    ))
}

/// `attached` as the wire carries them, then each artifact link naming a
/// file in the artifacts folder that isn't attached already. A link to a
/// missing file, a URL or a path outside the folder stays a link only.
pub fn with_links(
    attached: Vec<ItemAttachment>,
    links: &[ItemLink],
    artifacts: &Path,
) -> Vec<c::ItemAttachment> {
    let mut shown: HashSet<String> = attached
        .iter()
        .filter(|a| !a.removed && !a.path.is_empty())
        .map(|a| a.path.clone())
        .collect();
    let mut out: Vec<c::ItemAttachment> = attached
        .into_iter()
        .map(|a| to_wire(a, artifacts))
        .collect();
    let roots = roots(artifacts);
    for link in links.iter().filter(|l| l.kind == LinkKind::Artifact) {
        let Some(rel) = rel_in(&roots, &link.target) else {
            continue;
        };
        let path = rel
            .split('/')
            .fold(artifacts.to_path_buf(), |p, s| p.join(s));
        let Some(meta) = path.metadata().ok().filter(|m| m.is_file()) else {
            continue;
        };
        if !shown.insert(rel.clone()) {
            continue;
        }
        let name = link
            .label
            .clone()
            .filter(|l| !l.trim().is_empty())
            .unwrap_or_else(|| rel.rsplit('/').next().unwrap_or(&rel).to_string());
        out.push(c::ItemAttachment {
            id: format!("{LINK_ID_PREFIX}{}", link.target),
            item_id: link.item_id.clone(),
            kind: "doc".to_string(),
            name,
            mime: String::new(),
            bytes: meta.len(),
            sha256: String::new(),
            path: path.display().to_string(),
            url: String::new(),
            caption: String::new(),
            added_by: link.created_by.clone(),
            at: stamp(link.at),
            removed: false,
        });
    }
    out
}

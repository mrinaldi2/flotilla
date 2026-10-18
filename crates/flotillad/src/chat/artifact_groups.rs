//! Artifacts by kind (0.22.0, `artifacts_v2`): which group a file in the
//! project's artifacts belongs to, and which live cards use it. The storage
//! folder of card attachments (`items/`) is never listed; dot folders such
//! as the linked computer's `.cache/` never were.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bus::Project;

use super::files::{artifacts_dir, Artifact};
use crate::app::AppState;

/// Where card attachments are stored, under the artifacts folder.
pub const ITEMS_DIR: &str = "items";
const UPLOADS_DIR: &str = "uploads";
const CHECKS_DIR: &str = "checks";

/// What kind of project file an artifact is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// Files no single card owns: everything not in another group.
    Shared,
    /// The owner's, under `uploads/`.
    Uploads,
    /// Check logs, under `checks/`.
    Checks,
}

impl Group {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "shared" => Some(Self::Shared),
            "uploads" => Some(Self::Uploads),
            "checks" => Some(Self::Checks),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Uploads => "uploads",
            Self::Checks => "checks",
        }
    }

    /// The group of a file by its `/`-separated path in the artifacts folder.
    pub fn of(rel: &str) -> Self {
        match rel.split_once('/') {
            Some((UPLOADS_DIR, _)) => Self::Uploads,
            Some((CHECKS_DIR, _)) => Self::Checks,
            _ => Self::Shared,
        }
    }

    /// Whether a top-level folder of the artifacts can hold this group's
    /// files.
    pub fn in_folder(self, folder: &str) -> bool {
        self == Self::of(&format!("{folder}/"))
    }
}

/// The artifacts folder as a card may name it: as configured, and resolved
/// (a temporary folder on macOS lives under a link).
pub fn roots(artifacts: &Path) -> Vec<PathBuf> {
    let mut roots = vec![artifacts.to_path_buf()];
    roots.extend(artifacts.canonicalize().ok().filter(|c| c != artifacts));
    roots
}

/// An artifact's `/`-separated path in the artifacts folder, from how a card
/// names it: relative to the folder, or absolute under it. `None` for
/// anything else, such as a URL, a path elsewhere or one that climbs out.
pub fn rel_in(roots: &[PathBuf], target: &str) -> Option<String> {
    let target = target.trim();
    if target.is_empty() || target.contains("://") {
        return None;
    }
    let path = Path::new(target);
    // `/x` counts as absolute on Windows too: it is never a relative name.
    let rel = if path.is_absolute() || path.has_root() {
        roots
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())?
            .to_string_lossy()
            .into_owned()
    } else {
        target.to_string()
    };
    let parts: Vec<&str> = rel
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();
    let escapes = parts.iter().any(|p| *p == ".." || p.contains(':'));
    (!parts.is_empty() && !escapes).then(|| parts.join("/"))
}

/// The live cards that use each artifact, by its path in the artifacts
/// folder: a card attaches it, or links it with an `artifact` link.
pub fn used_by(app: &AppState, project: &Project) -> anyhow::Result<BTreeMap<String, Vec<String>>> {
    let roots = roots(&artifacts_dir(app, project));
    let uses = app.db.board_read(|t| t.artifact_uses(&project.id))?;
    let mut by_path: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (item_id, target) in uses {
        let Some(rel) = rel_in(&roots, &target) else {
            continue;
        };
        let cards = by_path.entry(rel).or_default();
        if !cards.contains(&item_id) {
            cards.push(item_id);
        }
    }
    Ok(by_path)
}

/// Fills each artifact's `used_by` and `kept`. A listing still answers when
/// the board can't be read: its files just show no cards.
pub fn fill_uses(app: &AppState, project: &Project, artifacts: &mut [Artifact]) {
    let by_path = used_by(app, project).unwrap_or_else(|e| {
        tracing::warn!(project = %project.id, "artifact uses: {e:#}");
        BTreeMap::new()
    });
    let kept = app.db.kept_artifacts(&project.id).unwrap_or_else(|e| {
        tracing::warn!(project = %project.id, "kept artifacts: {e:#}");
        Default::default()
    });
    for artifact in artifacts {
        artifact.used_by = by_path.get(&artifact.rel).cloned().unwrap_or_default();
        artifact.kept = kept.contains(&artifact.rel);
    }
}

/// An owner request's artifact path, `/`-separated: relative to the
/// artifacts folder, in a folder a listing shows (not `items/` or a dot
/// folder) and never climbing out of it.
pub fn listed_rel(path: &str) -> anyhow::Result<String> {
    let rel = rel_in(&[], path)
        .filter(|rel| {
            !rel.split('/').any(|p| p.starts_with('.'))
                && rel.split_once('/').is_none_or(|(top, _)| top != ITEMS_DIR)
        })
        .ok_or_else(|| {
            crate::decisions::invalid(format!(
                "{path} is not a listed artifact: name it by its path in the artifacts folder"
            ))
        })?;
    Ok(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_grouped_by_its_top_folder() {
        assert_eq!(Group::of("report.md"), Group::Shared);
        assert_eq!(Group::of("runs/1/frame.png"), Group::Shared);
        assert_eq!(Group::of("uploads/shot.png"), Group::Uploads);
        assert_eq!(Group::of("checks/ck-1/build.log"), Group::Checks);
        // A file named like a folder is still a shared file.
        assert_eq!(Group::of("uploads"), Group::Shared);
        assert!(Group::Shared.in_folder("runs"));
        assert!(!Group::Shared.in_folder("uploads"));
        assert!(Group::Checks.in_folder("checks"));
        assert!(!Group::Uploads.in_folder("checks"));
    }

    #[test]
    fn a_card_names_an_artifact_relative_or_absolute() {
        let root = PathBuf::from("/p/artifacts");
        let roots = [root.clone()];
        assert_eq!(rel_in(&roots, "spec.md").as_deref(), Some("spec.md"));
        assert_eq!(rel_in(&roots, "./runs\\a.md").as_deref(), Some("runs/a.md"));
        let absolute = root.join("runs").join("a.md");
        assert_eq!(
            rel_in(&roots, &absolute.display().to_string()).as_deref(),
            Some("runs/a.md")
        );
        for elsewhere in [
            "/p/repo/README.md",
            "https://x.dev/a",
            "../secret",
            "",
            "C:x",
        ] {
            assert_eq!(rel_in(&roots, elsewhere), None, "{elsewhere}");
        }
    }
}

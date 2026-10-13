//! `item_attach` and `item_detach` for bots (0.19.0 K4b). A bot attaches a
//! file of its own or of the project's artifacts (copied in under the
//! caps), links an artifact or a URL as a doc, or detaches one.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bus::contract::board as c;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::kanban::attach::{artifacts_of, Source};
use crate::kanban::attachments::to_wire;
use crate::kanban::edit::Editor;
use crate::peer::artifacts::allowed_roots;

use super::board::{out, Me};
use super::board_schema::decode;

pub(super) fn call(
    app: &Arc<AppState>,
    me: &Me,
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    let project = me.bot.project_id.as_str();
    let ed = Editor {
        app,
        project_id: project,
        actor: me.actor(),
    };
    if name == "item_detach" {
        let req: c::ItemDetach = decode("ItemDetach", args, project)?;
        let removed = ed.detach(&req)?;
        return Ok(json!({ "removed": removed }));
    }
    let req: c::ItemAttach = decode("ItemAttach", args, project)?;
    let given = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let source = match (given(&req.file), given(&req.artifact), given(&req.url)) {
        (Some(file), None, None) => Source::File(own_file(app, &me.bot, &file)?),
        (None, Some(artifact), None) => Source::Artifact(artifact),
        (None, None, Some(url)) => Source::Url(url),
        _ => anyhow::bail!("give one of 'file', 'artifact' or 'url'"),
    };
    anyhow::ensure!(
        req.upload_id.is_none(),
        "'upload_id' is for the owner's apps; attach a 'file' instead"
    );
    let att = ed.attach(&req, source)?;
    let artifacts = artifacts_of(app, project)?;
    Ok(json!({ "attachment": out(to_wire(att, &artifacts))? }))
}

/// A file the bot may hand over: in its own folder or the project's
/// artifacts, after links are resolved.
fn own_file(app: &Arc<AppState>, bot: &bus::Bot, file: &str) -> anyhow::Result<PathBuf> {
    let path = Path::new(&bot.workspace_path).join(file);
    let path = path
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("no file {file}"))?;
    anyhow::ensure!(
        allowed_roots(app, bot)
            .iter()
            .any(|root| path.starts_with(root)),
        "{file} is outside your workspace and the project's artifacts"
    );
    anyhow::ensure!(path.is_file(), "{file} is not a file");
    Ok(path)
}

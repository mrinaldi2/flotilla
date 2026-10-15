//! `item_attach` and `item_detach` for bots (0.19.0 K4b). A bot attaches a
//! file of its own or of the project's artifacts (copied in under the
//! caps), links an artifact or a URL as a doc, or detaches one.

use std::path::Path;
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
    // A linked computer's bot can't name a path here: its computer sends
    // the bytes first, and the home names their upload (0.19.0 K6).
    let linked = me.bot.is_linked();
    let upload = given(&req.upload_id).filter(|_| linked);
    let named = (given(&req.file), given(&req.artifact), given(&req.url));
    let source = match (named, upload) {
        ((Some(_), None, None), _) if linked => {
            anyhow::bail!("a file on a linked computer is sent by that computer; update it")
        }
        ((Some(file), None, None), None) => own_file(app, &me.bot, &file)?,
        ((None, Some(artifact), None), None) => Source::Artifact(artifact),
        ((None, None, Some(url)), None) => Source::Url(url),
        ((None, None, None), Some(upload)) => Source::Upload(upload),
        _ => anyhow::bail!("give one of 'file', 'artifact' or 'url'"),
    };
    anyhow::ensure!(
        linked || req.upload_id.is_none(),
        "'upload_id' is for the owner's apps; attach a 'file' instead"
    );
    let att = ed.attach(&req, source)?;
    let artifacts = artifacts_of(app, project)?;
    Ok(json!({ "attachment": out(to_wire(att, &artifacts))? }))
}

/// A file the bot may hand over: in its own folder or the project's
/// artifacts, after links are resolved. The attach checks again once the
/// file is open, so a link swapped in meanwhile is refused too.
fn own_file(app: &Arc<AppState>, bot: &bus::Bot, file: &str) -> anyhow::Result<Source> {
    let path = Path::new(&bot.workspace_path).join(file);
    let path = path
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("no file {file}"))?;
    let roots = allowed_roots(app, bot);
    anyhow::ensure!(
        roots.iter().any(|root| path.starts_with(root)),
        "{file} is outside your workspace and the project's artifacts"
    );
    anyhow::ensure!(path.is_file(), "{file} is not a file");
    Ok(Source::File { path, roots })
}

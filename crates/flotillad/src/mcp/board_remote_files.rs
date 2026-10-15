//! An item's files for a bot whose board lives on a linked computer
//! (0.19.0 K6). `item_attach{file}` sends the file's bytes to the board's
//! home, which never reads a path on this computer, and `item_get` points
//! each image and file at a local copy fetched by its sha256
//! (`peer::files_cache`).

use std::path::Path;
use std::sync::Arc;

use bus::{Bot, ProjectLink};
use serde_json::{json, Value};

use crate::app::AppState;
use crate::peer::artifacts::allowed_roots;
use crate::peer::files_cache::{local_copy, push, Push, Wanted};

/// `item_attach` with a `file` of the bot's: its bytes go to the home as an
/// upload, then the attach names the upload.
pub(super) async fn attach(
    app: &Arc<AppState>,
    bot: &Bot,
    link: Option<&ProjectLink>,
    args: &Value,
) -> anyhow::Result<Value> {
    let link = link.ok_or_else(|| {
        anyhow::anyhow!(
            "this project's board lives on a linked computer this one hasn't heard from yet; \
             try again shortly"
        )
    })?;
    let text = |key: &str| args[key].as_str().map(str::trim).filter(|s| !s.is_empty());
    let file = text("file").unwrap_or_default();
    let item_id = text("id").ok_or_else(|| anyhow::anyhow!("'id' is required"))?;
    anyhow::ensure!(
        text("artifact").is_none() && text("url").is_none(),
        "give one of 'file', 'artifact' or 'url'"
    );
    // As an attach on the home checks it: in the bot's folder or the
    // project's artifacts, once links are resolved; checked again once open.
    let path = Path::new(&bot.workspace_path)
        .join(file)
        .canonicalize()
        .map_err(|_| anyhow::anyhow!("no file {file}"))?;
    let roots = allowed_roots(app, bot);
    anyhow::ensure!(
        roots.iter().any(|root| path.starts_with(root)),
        "{file} is outside your workspace and the project's artifacts"
    );
    let own = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    let name = text("name").map_or(own, str::to_string);
    let upload_id = push(
        app,
        &Push {
            link,
            bot_id: &bot.id,
            item_id,
            path: &path,
            roots: &roots,
            name: &name,
        },
    )
    .await?;
    let mut args = args.clone();
    if let Some(fields) = args.as_object_mut() {
        fields.remove("file");
        fields.insert("upload_id".into(), json!(upload_id));
        fields.insert("name".into(), json!(name));
    }
    Ok(args)
}

/// Points the attachments of an `item_get` (or the one of an
/// `item_attach`) at local copies, and the description's references with
/// them. One that can't be copied here loses the home's path and says why,
/// and the description names it `attachment:<id>` again.
pub(super) async fn localize(app: &AppState, link: &ProjectLink, result: &mut Value) {
    let home = crate::peer::board::home_name(app, &link.peer_id);
    let item_cap = app.cfg.attachments.item_mb * 1024 * 1024;
    let mut copied = 0u64;
    let mut moved = Vec::new();
    let atts: Vec<&mut Value> = match result.get_mut("attachments") {
        Some(Value::Array(list)) => list.iter_mut().collect(),
        _ => result.get_mut("attachment").into_iter().collect(),
    };
    for att in atts {
        let Some(old) = att["path"].as_str().map(str::to_string) else {
            continue;
        };
        let text = |key: &str| att[key].as_str().unwrap_or_default().to_string();
        let (kind, removed) = (text("kind"), att["removed"].as_bool().unwrap_or(false));
        // A u64 in proto JSON is a string.
        let bytes = att["bytes"]
            .as_u64()
            .or_else(|| att["bytes"].as_str().and_then(|b| b.parse().ok()))
            .unwrap_or(0);
        let copy = if old.is_empty() || removed {
            Err(String::new())
        } else if kind.contains("doc") {
            Err(format!("a doc on {home}; ask a bot there for it"))
        } else if copied + bytes > item_cap {
            Err("over this computer's cap for one item's files".into())
        } else {
            let (id, name, sha256) = (text("id"), text("name"), text("sha256"));
            let want = Wanted {
                id: &id,
                name: &name,
                sha256: &sha256,
                bytes,
            };
            local_copy(app, link, &want)
                .await
                .map_err(|e| format!("not copied from {home}: {e:#}"))
        };
        match copy {
            Ok(path) => {
                copied += bytes;
                let path = path.display().to_string();
                moved.push((old, path.clone()));
                att["path"] = json!(path);
            }
            Err(why) => {
                // The description names it as the home does, not by a path
                // that means nothing here; the attachment's note says why.
                if !old.is_empty() {
                    moved.push((old, format!("attachment:{}", text("id"))));
                }
                att["path"] = json!("");
                if !why.is_empty() {
                    att["note"] = json!(why);
                }
            }
        }
    }
    if let Some(description) = result.pointer_mut("/item/description") {
        let mut text = description.as_str().unwrap_or_default().to_string();
        for (old, new) in &moved {
            text = text.replace(&format!("]({old})"), &format!("]({new})"));
        }
        *description = json!(text);
    }
}

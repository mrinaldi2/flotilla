//! What deleting a project moves to the trash, and the cleanup of projects
//! deleted before (0.19.0). All three are the owner's alone: the app's
//! ticket or a paired device, never the owner token a bot can read. They
//! read folders from disk, so they are answered off the connection's task.

use serde_json::{json, Value};

use super::Conn;
use crate::project_trash::{collect, past, peer};

pub(super) const KINDS: &[&str] = &[
    "project_delete_preview",
    "deleted_projects_preview",
    "deleted_projects_cleanup",
];

impl Conn {
    pub(super) fn trash_request(
        &self,
        kind: &str,
        req_id: &Value,
        req: &Value,
    ) -> anyhow::Result<()> {
        let app = self.app.clone();
        match kind {
            "project_delete_preview" => {
                let project_id = Self::str_field(req, "project_id")?;
                let Some(project) = self.app.db.get_live_project(project_id)? else {
                    self.reply_err(req_id, "not_found", "project not found or already deleted");
                    return Ok(());
                };
                self.spawn_blocking_reply(req_id, move |req_id| {
                    let mut preview = collect::collect(&app, &project).view();
                    preview["bin"] =
                        json!(crate::trash::bin_name(crate::trash::first_bin(&app.cfg)));
                    preview["peers"] = json!(peer::peers_view(&app, &project));
                    json!({ "type": "project_delete_preview", "req_id": req_id, "preview": preview })
                });
            }
            "deleted_projects_preview" => {
                self.spawn_blocking_reply(req_id, move |req_id| {
                    json!({
                        "type": "deleted_projects_preview", "req_id": req_id,
                        "preview": past::view(&app),
                    })
                });
            }
            _ => {
                self.spawn_blocking_reply(req_id, move |req_id| {
                    let report = past::clean(&app);
                    let mut reply = report.view();
                    reply["type"] = json!("deleted_projects_cleaned");
                    reply["req_id"] = req_id;
                    reply["summary"] = json!(report.sentence());
                    reply
                });
            }
        }
        Ok(())
    }
}

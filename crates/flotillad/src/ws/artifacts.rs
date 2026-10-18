//! The owner's tidy-up of loose artifacts (0.22.0, `artifacts_v2`): keep a
//! file no card uses, or move it to the trash. Nothing is removed on its
//! own; both are the owner's call, under the `approve` grant.

use std::sync::Arc;

use serde_json::{json, Value};

use super::chat::live_project;
use super::Conn;
use crate::app::AppState;
use crate::chat::artifact_groups::{self, listed_rel};
use crate::chat::files;
use crate::decisions::{conflict, error_code, not_found};
use crate::trash;

impl Conn {
    /// `artifact_trash {project_id, path}`: moves a loose artifact to the
    /// OS trash (or the trash folder), refused while a live card uses it.
    pub(super) fn artifact_trash(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let project_id = Self::str_field(req, "project_id")?.to_string();
        let rel = listed_rel(Self::str_field(req, "path")?)?;
        self.tidy(req_id, move |app| {
            let project = live_project(app, &project_id)?;
            let root = files::artifacts_dir(app, &project);
            let path = rel.split('/').fold(root.clone(), |p, s| p.join(s));
            if !path.is_file() {
                return Err(not_found(format!("no artifact {rel}")));
            }
            let cards = artifact_groups::used_by(app, &project)?
                .remove(&rel)
                .unwrap_or_default();
            if !cards.is_empty() {
                return Err(conflict(format!(
                    "{rel} is used by {}: take it off those cards first",
                    cards.join(", ")
                )));
            }
            trash::confine::check(&path, &root, trash::confine::Depth::Below)
                .map_err(crate::decisions::invalid)?;
            let bucket = trash::Bucket {
                label: &project.dir_name,
                group: "artifacts",
            };
            let bin = trash::move_to_trash(&app.cfg, &path, bucket).map_err(anyhow::Error::msg)?;
            app.db.set_artifact_kept(&project.id, &rel, false)?;
            Ok(json!({
                "type": "artifact_trashed", "project_id": project_id, "path": rel, "bin": bin
            }))
        });
        Ok(())
    }

    /// `artifact_keep {project_id, path, keep}`: marks a loose artifact kept,
    /// so it leaves the tidy-up list, or takes the mark off.
    pub(super) fn artifact_keep(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let project_id = Self::str_field(req, "project_id")?.to_string();
        let rel = listed_rel(Self::str_field(req, "path")?)?;
        let keep = req
            .get("keep")
            .and_then(Value::as_bool)
            .ok_or_else(|| crate::decisions::invalid("'keep' is required: true or false"))?;
        self.tidy(req_id, move |app| {
            let project = live_project(app, &project_id)?;
            let root = files::artifacts_dir(app, &project);
            let artifact = files::artifact_at(&root, &rel)
                .ok_or_else(|| not_found(format!("no artifact {rel}")))?;
            app.db.set_artifact_kept(&project.id, &rel, keep)?;
            let mut one = [artifact];
            crate::chat::authors::attribute(app, &project, &mut one);
            artifact_groups::fill_uses(app, &project, &mut one);
            let [artifact] = one;
            Ok(json!({ "type": "artifact_kept", "project_id": project_id, "artifact": artifact }))
        });
        Ok(())
    }

    /// Runs `work` off the connection's task, answering a refusal with its
    /// own code (`not_found`, `conflict`, `invalid_request`).
    fn tidy(
        &self,
        req_id: &Value,
        work: impl FnOnce(&Arc<AppState>) -> anyhow::Result<Value> + Send + 'static,
    ) {
        let app = self.app.clone();
        self.spawn_blocking_reply(req_id, move |req_id| match work(&app) {
            Ok(mut reply) => {
                reply["req_id"] = req_id;
                reply
            }
            Err(e) => json!({
                "type": "error", "req_id": req_id,
                "code": error_code(&e).unwrap_or("internal"), "message": format!("{e:#}")
            }),
        });
    }
}

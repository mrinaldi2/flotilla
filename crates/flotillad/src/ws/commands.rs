//! The commands a bot is running and has run, for its Commands panel. See
//! `crate::chat::commands`. Every bot's, for the Commands page, and the
//! owner's Stop (0.24.0): see `crate::commands`.

use serde_json::{json, Value};

use super::Conn;

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 500;

impl Conn {
    pub(super) fn list_bot_commands(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let bot_id = Self::str_field(req, "bot_id")?.to_string();
        let limit = req
            .get("limit")
            .and_then(Value::as_u64)
            .map_or(DEFAULT_LIMIT, |n| (n as usize).clamp(1, MAX_LIMIT));
        // A linked bot runs its commands on its machine, which reads them.
        if let Some(bot) = self.linked(&bot_id) {
            let frame = json!({ "type": "bot_commands", "limit": limit });
            self.remote(req_id, bot, frame, move |result| {
                json!({ "type": "bot_commands", "bot_id": bot_id, "commands": result["commands"] })
            });
            return Ok(());
        }
        self.blocking(req_id, move |app| {
            let bot = app
                .db
                .get_live_bot(&bot_id)?
                .ok_or_else(|| anyhow::anyhow!("bot not found"))?;
            let commands = app.chat.commands(app, &bot, limit)?;
            Ok(json!({ "type": "bot_commands", "bot_id": bot_id, "commands": commands }))
        });
        Ok(())
    }

    /// `list_commands {project_id?, bot_id?, status?, query?, limit?,
    /// before?}`: every bot's commands, here and on linked computers.
    pub(super) fn list_commands(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let text = |key: &str| req.get(key).and_then(Value::as_str).map(str::to_string);
        let filter = crate::commands::Filter {
            project_id: text("project_id"),
            bot_id: text("bot_id"),
            status: text("status"),
            query: text("query"),
            limit: req.get("limit").and_then(Value::as_u64).map(|n| n as usize),
            before: text("before"),
        };
        if let Some(status) = filter.status.as_deref() {
            if !matches!(status, "running" | "failed" | "all") {
                self.reply_err(
                    req_id,
                    "invalid_request",
                    "status is running, failed or all",
                );
                return Ok(());
            }
        }
        let app = self.app.clone();
        self.spawn_reply(req_id, move |req_id| async move {
            let mut reply = match crate::commands::list(&app, filter).await {
                Ok(reply) => reply,
                Err(e) => json!({ "type": "error", "code": "internal",
                                  "message": format!("{e:#}") }),
            };
            reply["req_id"] = req_id;
            reply
        });
        Ok(())
    }

    /// `stop_command {bot_id, command_id}`: the owner's Stop. The device or
    /// ticket it needs is checked at dispatch (`owner_auth`).
    pub(super) fn stop_command(&self, req_id: &Value, req: &Value) -> anyhow::Result<()> {
        let bot_id = Self::str_field(req, "bot_id")?.to_string();
        let command_id = Self::str_field(req, "command_id")?.to_string();
        let (app, by) = (self.app.clone(), self.actor().as_stored());
        self.spawn_reply(req_id, move |req_id| async move {
            match crate::commands::stop::stop(&app, &bot_id, &command_id, &by).await {
                Ok(command) => json!({ "type": "ok", "req_id": req_id, "command": command }),
                Err(e) => {
                    let code = crate::decisions::error_code(&e)
                        .unwrap_or_else(|| crate::peer::error_code(&e, "internal"));
                    json!({ "type": "error", "req_id": req_id, "code": code,
                            "message": format!("{e:#}") })
                }
            }
        });
        Ok(())
    }
}

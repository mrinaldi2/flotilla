//! The team on demand (0.23.0): a bot's presence, the owner waking a bot
//! or sending it to sleep, and this computer's running count and queue.
//! A linked bot is woken or sent to sleep on its own computer.

use serde_json::{json, Value};

use super::Conn;
use crate::app::AppState;
use crate::events::Push;
use crate::supervisor::{WakeReason, WakeRequest};
use bus::{Bot, BotPresence};

/// The requests this module serves.
pub(super) const KINDS: &[&str] = &["set_bot_presence", "wake_bot", "sleep_bot", "team_status"];

/// Whether `bot` leads its project, and so is always on.
fn is_lead(app: &AppState, bot: &Bot) -> anyhow::Result<bool> {
    Ok(app
        .db
        .get_project(&bot.project_id)?
        .and_then(|p| p.lead_bot_id)
        .is_some_and(|lead| lead == bot.id))
}

/// The owner wakes `bot`: ahead of board work, or queued for a slot.
pub(crate) fn wake(app: &AppState, bot: &Bot) {
    app.supervisor
        .wake(bot, WakeRequest::new(WakeReason::Owner));
}

/// The owner sends `bot` to sleep once its running turn ends. Refused for
/// an always-on bot.
pub(crate) fn sleep(app: &AppState, bot: &Bot) -> anyhow::Result<()> {
    if bot.presence == BotPresence::AlwaysOn || is_lead(app, bot)? {
        let why = format!("{} is always on: make it on demand first", bot.name);
        return Err(crate::decisions::conflict(why));
    }
    app.supervisor.request_sleep(&bot.id);
    Ok(())
}

impl Conn {
    pub(super) fn team_request(
        &self,
        kind: &str,
        req_id: &Value,
        req: &Value,
    ) -> anyhow::Result<()> {
        if kind == "team_status" {
            let team = self.app.supervisor.team_status();
            self.send(json!({ "type": "team", "req_id": req_id, "team": team }));
            return Ok(());
        }
        let bot_id = Self::str_field(req, "bot_id")?;
        let Some(bot) = self.app.db.get_live_bot(bot_id)? else {
            self.reply_err(req_id, "not_found", "bot not found");
            return Ok(());
        };
        if kind == "set_bot_presence" {
            return self.set_bot_presence(req_id, req, bot);
        }
        if bot.is_linked() {
            let frame = json!({ "type": kind });
            self.remote(req_id, bot, frame, |_| json!({ "type": "ok" }));
            return Ok(());
        }
        if kind == "sleep_bot" {
            sleep(&self.app, &bot)?;
        } else {
            wake(&self.app, &bot);
        }
        self.send(json!({ "type": "ok", "req_id": req_id }));
        Ok(())
    }

    fn set_bot_presence(&self, req_id: &Value, req: &Value, bot: Bot) -> anyhow::Result<()> {
        let presence = match req.get("presence").and_then(Value::as_str) {
            Some("always_on") => BotPresence::AlwaysOn,
            Some("on_demand") => BotPresence::OnDemand,
            _ => {
                let why = "'presence' must be always_on or on_demand";
                self.reply_err(req_id, "invalid_request", why);
                return Ok(());
            }
        };
        if bot.is_linked() {
            let why = format!("{} runs on another computer: set it there", bot.name);
            self.reply_err(req_id, "conflict", &why);
            return Ok(());
        }
        if presence == BotPresence::OnDemand && is_lead(&self.app, &bot)? {
            let why = format!("{} leads its project, and a lead is always on", bot.name);
            self.reply_err(req_id, "conflict", &why);
            return Ok(());
        }
        self.app.db.set_bot_presence(&bot.id, presence)?;
        let bot = self
            .app
            .db
            .get_live_bot(&bot.id)?
            .ok_or_else(|| anyhow::anyhow!("bot not found"))?;
        // An always-on bot asks for a slot at the next supervision tick; an
        // on-demand one sleeps once idle.
        self.app.events.push(Push::BotUpdated { bot: bot.clone() });
        self.send(json!({ "type": "bot", "req_id": req_id, "bot": self.bot_json(&bot) }));
        Ok(())
    }
}

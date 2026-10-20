//! A browser of each bot's own. A bot whose work needs one (see [`wanted`])
//! gets the Playwright MCP server driving a Chrome with a profile in the
//! bot's directory, headless by default, so bots never open tabs in the
//! owner's Chrome. The daemon finds a running browser through its
//! DevToolsActivePort file, streams it to the app, and passes the owner's
//! mouse and keyboard back to it. See "Browser" in `docs/protocol.md`.

mod cdp;
pub mod input;
pub mod setup;
pub mod streams;
pub mod view;

pub use setup::{BotBrowser, BrowserConfig};

use crate::db::Db;
use crate::kanban::model::Role;

/// Whether the bot's sessions get a browser of their own (0.23.0): a tester
/// or a UX reviewer, or a bot the owner allowed their own Chrome. Each
/// browser costs about 85 MB, so the rest of the team starts without one.
/// Roles are a board's: a project with no board here (none yet, or its
/// board lives on another computer, which keeps the roles) leaves every bot
/// its browser, as before.
pub fn wanted(db: &Db, bot: &bus::Bot) -> bool {
    if bot.user_chrome {
        return true;
    }
    let project = bot.project_id.as_str();
    let board_here = db.board_settings(project).ok().flatten().is_some()
        && db.board_home(project).ok().flatten().is_none();
    if !board_here {
        return true;
    }
    db.project_roles(&bot.project_id).is_ok_and(|roles| {
        roles
            .iter()
            .any(|r| r.bot_id == bot.id && matches!(r.role, Role::Tester | Role::ReviewerUx))
    })
}

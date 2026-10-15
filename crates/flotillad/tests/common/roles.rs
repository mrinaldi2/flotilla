//! The roles the tests' usual bot names stand for. The daemon guesses no
//! role from a bot's name (0.19.0 K8): the project's lead leads and the
//! owner gives every other role, so a test says who is who, as the owner
//! would, right after the board starts.

use flotillad::db::Db;
use flotillad::kanban::model::{ProjectRole, Role};

/// The role a test bot's name stands for, if any.
fn role_of(name: &str) -> Option<Role> {
    let name = name.trim().to_lowercase();
    match name.as_str() {
        "team lead" => Some(Role::Lead),
        "scrum master" => Some(Role::Coach),
        "devops" => Some(Role::Devops),
        "architect" => Some(Role::ReviewerArch),
        "context engineer" => Some(Role::ReviewerCe),
        "ux designer" => Some(Role::ReviewerUx),
        "desktop dev" | "ios dev" => Some(Role::Dev),
        _ if name.starts_with("tester") => Some(Role::Tester),
        _ => None,
    }
}

/// Gives each bot of the project the role its name stands for; a tester on
/// a linked computer tests on that computer.
pub fn by_name(db: &Db, project_id: &str) {
    for bot in db.list_bots(Some(project_id)).expect("bots") {
        let Some(role) = role_of(&bot.name) else {
            continue;
        };
        let machine = match (&bot.peer_id, role) {
            (Some(peer), Role::Tester) => db.get_peer(peer).expect("peer").map(|p| p.name),
            _ => None,
        };
        db.set_project_role(&ProjectRole {
            project_id: project_id.to_string(),
            role,
            bot_id: bot.id,
            machine,
        })
        .expect("role");
    }
}

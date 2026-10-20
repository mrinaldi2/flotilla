//! A browser only where the work needs one (0.23.0 step 3): on a project
//! with a board, a tester and a UX reviewer get the Playwright server and
//! the rest of the team starts without it, unless the owner allowed a bot
//! their own Chrome. Watching a bot without one shows no tabs.

use crate::common;

use bus::contract::board as c;
use common::peers::{project, wait_until};
use common::*;
use flotillad::project_roles::{set_role, Assigner};
use serde_json::{json, Value};

fn bot_root(d: &TestDaemon, bot_id: &str) -> std::path::PathBuf {
    let bot = d.app.db.get_bot(bot_id).expect("db").expect("bot");
    std::path::PathBuf::from(&bot.workspace_path)
        .parent()
        .expect("root")
        .to_path_buf()
}

fn has_browser(d: &TestDaemon, bot_id: &str) -> bool {
    let raw = std::fs::read_to_string(bot_root(d, bot_id).join("mcp.json")).expect("mcp.json");
    let mcp: Value = serde_json::from_str(&raw).expect("json");
    !mcp["mcpServers"]["playwright"].is_null()
}

fn prompt_says_browser(d: &TestDaemon, bot_id: &str) -> bool {
    std::fs::read_to_string(bot_root(d, bot_id).join("system.md"))
        .expect("system.md")
        .contains("browser of your own")
}

/// Restarts the bots and waits for their next sessions, which reread their
/// roles.
async fn restart(d: &TestDaemon, bots: &[&String]) {
    let starts: Vec<usize> = bots
        .iter()
        .map(|id| terminal(d, id).matches("chrome").count())
        .collect();
    for id in bots {
        d.app.supervisor.restart_bot(id).expect("restart");
    }
    wait_until("the bots start again", || {
        bots.iter()
            .zip(&starts)
            .all(|(id, n)| terminal(d, id).matches("chrome").count() > *n)
    })
    .await;
}

#[tokio::test]
async fn only_testers_and_ux_reviewers_get_a_browser() {
    let d = spawn_daemon_with(|cfg| {
        cfg.user_home = cfg.home.join("user");
        cfg.browser.node_dir = Some(cfg.home.join("node"));
    })
    .await;
    let mut c = WsClient::connect(&d).await;
    let pid = project(&mut c, "web").await;
    let mut ids = Vec::new();
    for name in ["lead", "qa", "ux", "dev"] {
        let bot = create_bot(&mut c, &pid, name).await;
        ids.push(bot["id"].as_str().expect("id").to_string());
    }
    let [_, qa, ux, dev] = &ids[..] else {
        unreachable!()
    };
    // No board yet: nobody holds a role, so everyone keeps a browser.
    wait_until("the dev starts", || {
        terminal(&d, dev).contains("--no-chrome")
    })
    .await;
    assert!(has_browser(&d, dev));

    d.app
        .db
        .ensure_board(&pid, &d.app.db.daemon_id().unwrap(), Some("W"))
        .unwrap();
    for (bot, role) in [
        ("qa", c::Role::Tester),
        ("ux", c::Role::ReviewerUx),
        ("dev", c::Role::Dev),
    ] {
        let req = c::RoleSet {
            project_id: pid.clone(),
            bot: bot.to_string(),
            role: role as i32,
            machine: None,
            remove: None,
        };
        set_role(&d.app, &pid, &req, Assigner::Owner { proved: true }).expect("role");
    }
    restart(&d, &[qa, ux, dev]).await;
    assert!(has_browser(&d, qa));
    assert!(has_browser(&d, ux));
    assert!(!has_browser(&d, dev));
    assert!(prompt_says_browser(&d, qa));
    assert!(!prompt_says_browser(&d, dev));

    // Watching the dev's browser shows that it has no tabs.
    let ok = c
        .request(json!({"type": "watch_browser", "bot_id": dev}))
        .await;
    assert_eq!(ok["type"], "ok", "{ok}");
    let tabs = c.wait_for(|v| v["type"] == "browser_tabs").await;
    assert_eq!(tabs["open"], false);

    // The owner allowing their own Chrome is asking for a browser too.
    let allowed = c
        .request(json!({"type": "set_bot_user_chrome", "bot_id": dev, "enabled": true}))
        .await;
    assert_eq!(allowed["type"], "bot", "{allowed}");
    wait_until("the dev restarts with the owner's Chrome", || {
        terminal(&d, dev).contains("--chrome ")
    })
    .await;
    assert!(has_browser(&d, dev));
}

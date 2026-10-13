//! The 0.19.0 acceptance scenario (K9, docs/kanban-rework.md), end to end on
//! two linked computers: "Recipe Box", homed on the Mac, with its lead
//! there and its cook on the PC. The owner uses the desktop app (the app's
//! ticket) and the phone (a paired device); the bots are MCP clients.
//!
//! What the product can't do yet is kept as ignored tests named `k9_gap_*`,
//! which say what they wait for.

use crate::common;

use bus::contract::board::{self as c, board_request::Request};
use bus::contract::home::AttentionKind;
use common::board::call;
use common::k9::*;
use serde_json::json;

/// Step 2: the lead proposes a Simple board under `owner_confirms`; the
/// owner approves it in Needs you. A fresh project's first board is the
/// release-gated Main the owner starts (gap G1), so the owner then makes
/// the new board the default and archives Main.
async fn step2(k: &mut K9) -> c::BoardSnapshot {
    let reply = k
        .lead
        .call(
            "board_create",
            json!({"name": "Recipes", "key": "RC", "preset": "simple"}),
        )
        .await;
    let proposal = proposed(&reply);
    let s = k.desktop_board().await;
    assert!(
        !s.boards.iter().any(|b| b.name == "Recipes"),
        "nothing changes before the owner rules"
    );
    let open = s
        .proposals
        .iter()
        .find(|p| p.id == proposal)
        .expect("shown");
    assert_eq!(open.state, "open");
    let rows = hermesd::overview::attention_rows(&k.p.mac.app, &k.mac_rb).unwrap();
    assert!(
        rows.rows
            .iter()
            .any(|r| r.kind == AttentionKind::Decision as i32
                && r.title.contains("create board “Recipes”")),
        "a Needs you row: {:?}",
        rows.rows
    );
    let s = approve(&mut k.p.mac_client, &proposal).await;
    let approved = k.lead_heard("Approved: create board “Recipes”").await;
    assert!(approved.contains("from USER"), "{approved}");

    let recipes = board(&s, "Recipes").clone();
    let s = applied(
        call(
            &mut k.p.mac_client,
            Request::BoardUpdate(c::BoardUpdate {
                board_id: recipes.id.clone(),
                version: recipes.version,
                is_default: Some(true),
                ..Default::default()
            }),
        )
        .await,
    );
    let main = board(&s, "Main").clone();
    assert_eq!(main.flow, "release_gated");
    applied(
        call(
            &mut k.p.mac_client,
            Request::BoardUpdate(c::BoardUpdate {
                board_id: main.id.clone(),
                version: main.version,
                archived: Some(true),
                ..Default::default()
            }),
        )
        .await,
    )
}

/// Step 3: the owner adds QA by hand in the desktop app; the lead is told
/// and its next `board_get` shows it. The lead also gives the board a
/// label group, a change it makes directly.
async fn step3(k: &mut K9, s: &c::BoardSnapshot) -> c::BoardSnapshot {
    let recipes = board(s, "Recipes").clone();
    let added = applied(
        call(
            &mut k.p.mac_client,
            Request::ColumnAdd(c::ColumnAdd {
                board_id: recipes.id.clone(),
                version: recipes.version,
                name: "QA".into(),
                kind: c::ColumnKind::Active as i32,
                after: Some(column(s, &recipes.id, "Doing")),
                rules: vec!["ac_all_checked".into()],
                ..Default::default()
            }),
        )
        .await,
    );
    let qa = column(&added, &recipes.id, "QA");
    let rules = added
        .columns
        .iter()
        .find(|col| col.key == qa)
        .map(|col| &col.rules);
    assert_eq!(rules, Some(&vec!["ac_all_checked".to_string()]));
    let told = k.lead_heard("add column “QA”").await;
    assert!(told.contains("board_get"), "{told}");
    let got = k.lead.call("board_get", json!({})).await;
    assert_eq!(got["key"], "RC", "the default board's prefix: {got}");
    let columns: Vec<&str> = got["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .filter_map(|col| col["name"].as_str())
        .collect();
    assert_eq!(columns, ["To do", "Doing", "QA", "Done"], "{got}");

    let listed = k.lead.call("board_list", json!({})).await;
    let version = &listed["boards"][0]["board"]["version"];
    let groups = json!([{"name": "course", "values": ["main", "dessert"]}]);
    let reply = k
        .lead
        .call(
            "board_update",
            json!({"board_id": recipes.id, "version": version, "label_groups": groups}),
        )
        .await;
    assert_eq!(reply["applied"], true, "{reply}");
    let s = k.desktop_board().await;
    let groups = &board(&s, "Recipes").label_groups;
    assert_eq!(groups.len(), 1, "{groups:?}");
    s
}

#[tokio::test]
async fn k9_recipe_box_end_to_end() {
    // 1. A fresh project and two bots, one the lead; the board started on
    // the Mac, which the PC is linked to.
    let mut k = setup().await;
    let s = step2(&mut k).await;
    let s = step3(&mut k, &s).await;
    let recipes = board(&s, "Recipes").id.clone();

    // 4. A draft from the phone, shaped with the lead, finalized on the
    // desktop; the cook's task lists the screenshot and the spec.
    let todo = column(&s, &recipes, "To do");
    let id = drafts::step4(&mut k, &recipes, &todo).await;
    assert!(id.starts_with("RC-"), "{id}");

    // 5. To do → Doing → QA → Done, refused once naming the missing rule.
    flow::step5(&mut k, &s, &recipes, &id).await;

    // 6. The Research board, from the phone's approval of a proposal.
    let research = flow::step6(&mut k).await;

    // 7. The cook moves an item from the PC; with the home offline its
    // reads work and its writes name the home.
    let research_item = flow::step7(&mut k, &research).await;

    // 8. Nothing in the project names desktop, iOS, the daemon or releases.
    let surfaces = k.kanban_surfaces(&[&id, &research_item]).await;
    assert!(surfaces.len() > 20, "every surface is read");
    let found = scan::report(&surfaces);
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// G1: a fresh project's first board is always the release-gated Main the
/// owner starts ("Start the board on this computer"); before it exists the
/// lead's board tools are refused. Step 2 as the plan writes it needs the
/// lead's `board_create` to make a project's first board, which the owner
/// approves, and the apps to show that proposal for a project with no board.
#[tokio::test]
#[ignore = "K9 gap G1: a project's first board can only be the release-gated Main the owner \
            starts; a lead's board_create needs a board to exist (design decision, see PR)"]
async fn k9_gap_the_leads_board_create_starts_a_fresh_projects_board() {
    let d = common::spawn_daemon().await;
    let mut desktop = common::WsClient::connect(&d).await;
    let project = common::peers::project(&mut desktop, "Recipe Box").await;
    let lead = common::create_bot(&mut desktop, &project, "Lead").await;
    let lead_id = lead["id"].as_str().expect("id");
    d.app.db.set_project_lead(&project, Some(lead_id)).unwrap();
    let token = d.app.secrets.bot_token(lead_id).expect("token");
    let mut lead = common::McpClient::new(&d, &token);
    let raw = lead
        .call_raw(
            "board_create",
            json!({"name": "Recipes", "key": "RB", "preset": "simple"}),
        )
        .await;
    assert_ne!(raw["isError"], json!(true), "{raw}");
    let reply: serde_json::Value =
        serde_json::from_str(raw["content"][0]["text"].as_str().unwrap_or("{}")).unwrap();
    let s = approve(&mut desktop, &proposed(&reply)).await;
    let flows: Vec<(&str, &str, &str)> = s
        .boards
        .iter()
        .map(|b| (b.name.as_str(), b.key.as_str(), b.flow.as_str()))
        .collect();
    assert_eq!(flows, [("Recipes", "RB", "custom")]);
}

/// G2: the prompt's bus sections and the tools of Projects & Bots and
/// Releases still name the daemon and releases, in every project: "the
/// daemon" authenticates messages, owner reporting names releases, and the
/// release tools are offered to every bot. Kanban's own text is clean
/// (step 8); the rest is the plan's "Removing the remaining
/// repository-specific values in Projects & Bots", out of scope for 0.19.0.
#[tokio::test]
#[ignore = "K9 gap G2: the bus prompt and the Projects & Bots and Releases tools still name the \
            daemon and releases; out of scope for 0.19.0 (see PR)"]
async fn k9_gap_nothing_a_bot_is_told_names_the_daemon_or_releases() {
    let mut k = setup().await;
    let surfaces = k.bot_surfaces().await;
    let found = scan::report(&surfaces);
    assert!(found.is_empty(), "{}", found.join("\n"));
}

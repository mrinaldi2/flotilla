//! A column's role nobody holds (0.24.0 step 2, `board_flow_v1`): the move
//! still succeeds and the card keeps its assignee, Needs you shows one
//! `ROLE_UNHELD` row per board and role, and once a bot holds the role the
//! waiting card goes to it and the row clears.

use crate::common;

use bus::contract::home::{attention_row::Target, AttentionKind};
use common::cards::fresh;
use serde_json::json;

/// The project's unheld-role rows: (role, title, weight).
fn unheld(d: &common::TestDaemon, project: &str) -> Vec<(String, String, u32)> {
    flotillad::overview::attention_rows(&d.app, project)
        .unwrap()
        .rows
        .into_iter()
        .filter(|r| r.kind == AttentionKind::RoleUnheld as i32)
        .map(|r| {
            let Some(Target::Role(role)) = r.target else {
                panic!("a role row targets its role: {r:?}");
            };
            (role, r.title, r.weight)
        })
        .collect()
}

#[tokio::test]
async fn a_role_nobody_holds_waits_in_needs_you_until_a_bot_holds_it() {
    let (mut f, _) = fresh(&["Lead", "Ari"]).await;
    let ari = f.bot_ids[1].clone();

    // Two new cards enter To do and nobody holds Product: one row.
    let first = f.item("Export orders").await;
    let second = f.item("Import orders").await;
    assert_eq!(f.now(&first.id).assignee, None);
    assert_eq!(
        unheld(&f.d, &f.project),
        [(
            "product".to_string(),
            format!(
                "Nobody holds Product: {} “Export orders” waits in To do",
                first.id
            ),
            2
        )]
    );

    // The lead moves the first to Research, whose Architect nobody holds:
    // the move succeeds, and the card stays with the lead.
    f.assign(&first.id, Some("Lead")).await;
    let version = f.now(&first.id).version;
    let moved = f.bots[0]
        .call_raw(
            "item_move",
            json!({"id": first.id, "to": "research", "expected_version": version}),
        )
        .await;
    assert_ne!(moved["isError"], json!(true), "{moved}");
    assert_eq!(f.now(&first.id).column_key, "research");
    assert_eq!(f.now(&first.id).assignee, Some(f.bot_ids[0].clone()));
    let rows = unheld(&f.d, &f.project);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert!(rows.contains(&(
        "product".to_string(),
        format!(
            "Nobody holds Product: {} “Import orders” waits in To do",
            second.id
        ),
        2
    )));
    assert!(rows.contains(&(
        "architect".to_string(),
        format!(
            "Nobody holds Architect: {} “Export orders” waits in Research",
            first.id
        ),
        2
    )));

    // The lead gives Ari the role: the waiting card goes to Ari, who is
    // told, and the Architect row clears.
    f.bots[0]
        .call("role_set", json!({"bot": "Ari", "role": "architect"}))
        .await;
    assert_eq!(f.now(&first.id).assignee, Some(ari));
    let told = f.heard(1, "where the Architect takes it").await;
    assert!(told.contains("“Code research”"), "{told}");
    let rows = unheld(&f.d, &f.project);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].0, "product");
}

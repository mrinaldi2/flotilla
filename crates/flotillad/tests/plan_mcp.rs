//! The plan over MCP (0.21.0, #43): the lead reads the plan and its
//! documents, proposes an initiative, asks to skip research and submits
//! once research is closed; rulings stay the owner's.

use serde_json::{json, Value};

use crate::common::plan::planned;
use crate::common::tasks::error_text;
use crate::common::McpClient;

fn names(tools: &Value) -> Vec<String> {
    tools["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|t| t["name"].as_str().expect("name").to_string())
        .collect()
}

#[tokio::test]
async fn the_lead_works_the_plan_over_mcp() {
    let plan = planned().await;
    let token = plan.d.app.secrets.bot_token(&plan.lead).expect("token");
    let mut lead = McpClient::new(&plan.d, &token);
    let tools = names(&lead.tools().await);
    for tool in [
        "plan_get",
        "plan_node_get",
        "plan_doc_read",
        "plan_propose",
        "plan_submit",
    ] {
        assert!(tools.contains(&tool.to_string()), "{tool}");
    }
    assert!(
        !tools.iter().any(|t| t == "plan_rule"),
        "rulings are the owner's"
    );

    let goal = plan.goal("Improve combat UX");
    let initiative = plan.initiative(&goal.id, "Combat readability");
    let tree = lead.call("plan_get", json!({})).await;
    assert_eq!(tree["enabled"], json!(true));
    assert_eq!(tree["nodes"].as_array().expect("nodes").len(), 2);

    let doc = lead
        .call(
            "plan_doc_read",
            json!({"path": "docs/guidelines/style.md", "offset": 0, "limit": 0}),
        )
        .await;
    assert_eq!(doc["text"], json!("# Style\n"));
    assert_eq!(doc["mime"], json!("text/markdown"));

    let raw = lead
        .call_raw(
            "plan_submit",
            json!({"id": initiative.id, "expected_version": initiative.version,
                   "doc_path": "docs/initiatives/combat/prd.md"}),
        )
        .await;
    assert!(error_text(&raw).contains("plan.research_open"), "{raw}");

    let asked = lead
        .call(
            "plan_research_skip",
            json!({"id": initiative.id, "expected_version": initiative.version,
                   "reason": "Playtest notes already cover it."}),
        )
        .await;
    assert_eq!(asked["node"]["gate"]["stage"], json!("research"));

    let proposed = lead
        .call(
            "plan_propose",
            json!({"parent_id": goal.id, "title": "Combat audio cues",
                   "summary": "Sound could warn players of attacks off screen."}),
        )
        .await;
    assert_eq!(proposed["node"]["stage"], json!("proposed"));
    assert_eq!(proposed["node"]["kind"], json!("initiative"));
}

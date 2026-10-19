//! Questions over the wire (0.22.0, `questions_v1`): a bot asks over MCP,
//! the owner lists and answers over WebSocket, and watchers hear each
//! change.

use std::time::Duration;

use bus::contract::board::{
    self as c, board_push::Push, board_request::Request, board_response::Response,
};
use bus::contract::wire::envelope::Body;
use serde_json::{json, Value};

use crate::common;
use common::board::{call, error_code, next_envelope, response};
use common::plan::*;
use common::tasks::error_text;
use common::McpClient;

/// The next `question_change` push, skipping the rest.
async fn question_change(c: &mut common::WsClient) -> c::QuestionChange {
    loop {
        let envelope = next_envelope(c, Duration::from_secs(3))
            .await
            .expect("a push");
        if let Some(Body::BoardPush(c::BoardPush {
            push: Some(Push::QuestionChange(change)),
        })) = envelope.body
        {
            return change;
        }
    }
}

fn names(tools: &Value) -> Vec<String> {
    tools["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|t| t["name"].as_str().expect("name").to_string())
        .collect()
}

#[tokio::test]
async fn a_bot_asks_over_mcp_and_the_owner_answers_over_websocket() {
    let mut plan = planned().await;
    let goal = plan.goal("A research of all saints");
    let init = plan.initiative(&goal.id, "Sources & licensing");
    let spike = plan.spike(&init, "Sources");
    let mut watcher = common::WsClient::connect(&plan.d).await;
    response(
        call(
            &mut watcher,
            Request::BoardWatch(c::BoardWatch {
                project_id: plan.pid.clone(),
            }),
        )
        .await,
    );

    let token = plan.d.app.secrets.bot_token(&plan.lead).expect("token");
    let mut lead = McpClient::new(&plan.d, &token);
    let tools = names(&lead.tools().await);
    assert!(tools.contains(&"ask_owner".to_string()));
    assert!(tools.contains(&"question_list".to_string()));
    assert!(
        !tools.iter().any(|t| t == "question_answer"),
        "answers are the owner's"
    );

    // `blocking` may be left out; at most eight at once.
    let too_many: Vec<Value> = (0..9).map(|i| json!({"text": format!("Q{i}?")})).collect();
    let raw = lead
        .call_raw(
            "ask_owner",
            json!({"item_id": spike, "questions": too_many}),
        )
        .await;
    assert!(error_text(&raw).contains("question.too_many"), "{raw}");
    let asked = lead
        .call(
            "ask_owner",
            json!({"item_id": spike, "questions": [
                {"text": "Only the celebrated saints?", "why": "It sets the size.",
                 "options": [{"key": "yes", "label": "Yes"}, {"key": "no", "label": "No"}],
                 "recommendation": "yes"}
            ]}),
        )
        .await;
    let id = asked["questions"][0]["id"]
        .as_str()
        .expect("an id")
        .to_string();
    assert_eq!(question_change(&mut watcher).await.question.unwrap().id, id);

    // The owner lists the node's questions and answers one.
    let Response::Questions(page) = response(
        call(
            &mut plan.owner,
            Request::QuestionList(c::QuestionList {
                project_id: plan.pid.clone(),
                plan_node_id: Some(init.id.clone()),
                ..Default::default()
            }),
        )
        .await,
    ) else {
        panic!("a page");
    };
    assert_eq!(page.questions.len(), 1);
    let q = &page.questions[0];
    let Response::Questioned(result) = response(
        call(
            &mut plan.owner,
            Request::QuestionAnswer(c::QuestionAnswer {
                id: q.id.clone(),
                expected_version: q.version,
                option: Some("yes".into()),
                text: String::new(),
            }),
        )
        .await,
    ) else {
        panic!("questioned");
    };
    assert!(matches!(
        result.outcome,
        Some(c::question_result::Outcome::Done(_))
    ));
    let change = question_change(&mut watcher).await;
    assert_eq!(
        change.question.unwrap().state,
        c::QuestionState::Answered as i32
    );

    // The card's detail carries it; the bot reads it answered.
    let Response::Item(detail) = response(
        call(
            &mut plan.owner,
            Request::ItemGet(c::ItemGet { id: spike.clone() }),
        )
        .await,
    ) else {
        panic!("an item");
    };
    assert_eq!(detail.questions.len(), 1);
    let listed = lead.call("question_list", json!({"item_id": spike})).await;
    assert_eq!(listed["questions"][0]["answer"]["text"], json!("Yes"));

    // Asking is a bot's, not the owner's.
    let refused = call(
        &mut plan.owner,
        Request::AskOwner(c::AskOwner {
            item_id: Some(spike),
            ..Default::default()
        }),
    )
    .await;
    assert_eq!(error_code(refused), "invalid_request");
}

#[tokio::test]
async fn the_turn_an_answer_starts_is_on_its_card_and_says_so() {
    let mut plan = planned().await;
    let goal = plan.goal("A research of all saints");
    let init = plan.initiative(&goal.id, "Sources & licensing");
    let spike = plan.spike(&init, "Sources");
    for _ in 0..400 {
        if plan.d.app.supervisor.state(&plan.lead).0 == bus::BotState::Ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        plan.d.app.supervisor.state(&plan.lead).0,
        bus::BotState::Ready
    );
    // The lead's own kickoff for the research card is delivered first, so
    // the answer's delivery is the newest when its turn starts.
    // The owner's board connection speaks the binary contract; the toasts
    // the apps show come as JSON pushes.
    let mut pushes = common::WsClient::connect(&plan.d).await;
    let token = plan.d.app.secrets.bot_token(&plan.lead).expect("token");
    let mut lead = McpClient::new(&plan.d, &token);
    let asked = lead
        .call(
            "ask_owner",
            json!({"item_id": spike, "questions": [
                {"text": "Italy only?", "why": "It sets which calendars we cite."}
            ]}),
        )
        .await;
    let id = asked["questions"][0]["id"].as_str().expect("an id");
    let Response::Questions(page) = response(
        call(
            &mut plan.owner,
            Request::QuestionList(c::QuestionList {
                project_id: plan.pid.clone(),
                item_id: Some(spike.clone()),
                ..Default::default()
            }),
        )
        .await,
    ) else {
        panic!("a page");
    };
    let q = page.questions.iter().find(|q| q.id == id).expect("asked");
    let Response::Questioned(result) = response(
        call(
            &mut plan.owner,
            Request::QuestionAnswer(c::QuestionAnswer {
                id: q.id.clone(),
                expected_version: q.version,
                option: None,
                text: "Italy and the universal calendar".into(),
            }),
        )
        .await,
    ) else {
        panic!("questioned");
    };
    assert!(matches!(
        result.outcome,
        Some(c::question_result::Outcome::Done(_))
    ));
    // The lead also hears of its research kickoff (another card): wait for
    // the answer's own delivery, so the turn below is the one it starts.
    let db = &plan.d.app.db;
    let conv = db
        .dm_conversation(&plan.lead)
        .unwrap()
        .expect("the lead's DM");
    let answer = db
        .list_messages(&conv.id, None, 100)
        .unwrap()
        .into_iter()
        .rev()
        .find(|m| m.decision_id.as_deref() == Some(id))
        .expect("the answer's message")
        .id;
    pushes
        .wait_for_within(Duration::from_secs(10), |v| {
            v["type"] == "delivery_update"
                && v["delivery"]["message_id"] == json!(answer)
                && v["delivery"]["state"] == "delivered"
        })
        .await;

    plan.d
        .app
        .supervisor
        .on_hook(&plan.lead, "UserPromptSubmit", None);
    let working = pushes
        .wait_for(|v| v["type"] == "bot_state" && v["state"] == "working")
        .await;
    assert_eq!(working["item_id"], json!(spike), "{working}");
    assert_eq!(working["cause"], "answer", "{working}");
    plan.d.app.supervisor.on_hook(&plan.lead, "Stop", None);
    let done = pushes
        .wait_for(|v| v["type"] == "bot_state" && v["state"] == "ready")
        .await;
    assert_eq!(done["item_id"], json!(spike), "{done}");
    assert!(done.get("cause").is_none(), "{done}");
}

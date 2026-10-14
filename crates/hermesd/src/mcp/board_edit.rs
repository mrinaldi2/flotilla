//! The board tools that change an item: thin over `kanban::edit`, which
//! the owner's WebSocket edits share. A refusal or a stale version is a tool
//! error naming it.

use std::sync::Arc;

use bus::contract::board as c;
use serde_json::{json, Value};

use crate::app::AppState;
use crate::kanban::edit::{Edited, Editor};
use crate::kanban::feed::ChangeKind;
use crate::kanban::model::{ColumnCategory, Item};

use super::board::{conflict, item_out, out, own_item, published, refused, Me};
use super::board_schema::decode;

pub(super) fn call(
    app: &Arc<AppState>,
    me: &Me,
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    let project = me.bot.project_id.as_str();
    let ed = Editor {
        app,
        project_id: project,
        actor: me.actor(),
    };
    let edited = match name {
        "item_create" => ed.create(&decode("ItemCreate", args, project)?)?,
        "item_update" => ed.update(&decode("ItemUpdate", args, project)?)?,
        "item_comment" => return comment(app, me, decode("ItemAddComment", args, project)?),
        "item_link" => {
            let link = ed.link(&decode("ItemAddLink", args, project)?)?;
            return Ok(json!({ "link": out(c::ItemLink::from(link))? }));
        }
        "item_unlink" => {
            let removed = ed.unlink(&decode("ItemRemoveLink", args, project)?)?;
            return Ok(json!({ "removed": removed }));
        }
        "item_block" => ed.block(&decode("ItemBlock", args, project)?)?,
        "item_unblock" => ed.unblock(&decode("ItemUnblock", args, project)?)?,
        "item_assign" => ed.assign(&decode("ItemAssign", args, project)?)?,
        "item_rank" => ed.rank(&decode("ItemRank", args, project)?)?,
        "item_check_ac" => {
            let sender = crate::mcp::bot_sender(&me.bot);
            ed.check_ac(&decode("ItemCheckAc", args, project)?, &sender)?
        }
        "item_flag_ac" => ed.flag_ac(&decode("ItemFlagAc", args, project)?)?,
        "item_attach" | "item_detach" => return super::board_attach::call(app, me, name, args),
        other => anyhow::bail!("unknown tool: {other}"),
    };
    reply(edited)
}

/// An edit's outcome as a tool result: the item, or a refusal or conflict
/// as a tool error.
pub(super) fn reply(edited: Edited) -> anyhow::Result<Value> {
    match edited {
        Edited::Done(item) => Ok(json!({ "item": item_out(*item)? })),
        Edited::Refused(unmet) => Err(refused(unmet)),
        Edited::Conflict(item) => Err(conflict(*item)),
    }
}

fn comment(app: &Arc<AppState>, me: &Me, req: c::ItemAddComment) -> anyhow::Result<Value> {
    own_item(app, me, &req.id)?;
    anyhow::ensure!(!req.body.trim().is_empty(), "'body' is empty");
    published(
        app,
        &me.bot.project_id,
        ChangeKind::ItemUpserted,
        None,
        || {
            let reply_to = req.reply_to.as_deref();
            let comment = app
                .db
                .add_item_comment(&req.id, &req.body, reply_to, &me.actor())?;
            // A question for the owner: kept here, on the board's home, so
            // their answer reaches the bot that asked (H-211).
            if req.asks_owner == Some(true) {
                app.db.add_card_question(&comment, &me.bot.id)?;
            }
            let out = json!({ "comment": out(c::ItemComment::from(comment))? });
            Ok((out, req.id.clone()))
        },
    )
}

/// An item_move reply. Into Verify it lists the criteria nobody has ticked,
/// so the mover sees what the testers still have to prove (H-116).
pub(super) fn moved(item: Item) -> anyhow::Result<Value> {
    let unticked: Vec<Value> = if item.category == ColumnCategory::Verify {
        item.acceptance_criteria
            .iter()
            .filter(|ac| !ac.checked)
            .map(|ac| json!({ "index": ac.idx, "text": ac.text, "post_install": ac.post_install }))
            .collect()
    } else {
        Vec::new()
    };
    let mut reply = json!({ "item": item_out(item)? });
    if !unticked.is_empty() {
        reply["unticked_ac"] = json!(unticked);
        reply["note"] = json!(
            "These acceptance criteria aren't ticked yet. Tick each one already verified \
             with item_check_ac; flag those provable only after install with item_flag_ac."
        );
    }
    Ok(reply)
}

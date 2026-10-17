//! The bots on the projects home (0.20.0, `overview_bots`): a `BotBrief`
//! per bot, read off its state on its own computer, and the one list a
//! project row shows across its computers.

use std::collections::{HashMap, HashSet};

use bus::contract::home::{project_attention::Part, BotBrief, BotStatus};
use bus::{Bot, BotState};

use crate::app::AppState;
use crate::attention::timestamp;
use crate::db::Db;

/// The most bots a row lists; `bots` stays the total.
pub(super) const BOTS_SHOWN: usize = 24;

/// Whether the bot's session runs: ready, at work, or waiting on the owner.
pub(super) fn running(state: BotState) -> bool {
    matches!(
        state,
        BotState::Ready
            | BotState::Working
            | BotState::WaitingForUser
            | BotState::WaitingForApproval
    )
}

/// The way a bot stopped that a `bot_stopped` row says, when `state` is
/// one; starting and stopping are on the way somewhere else.
pub(super) fn down_state(state: BotState) -> Option<BotState> {
    match state {
        BotState::Crashed | BotState::AuthFailed | BotState::RateLimited | BotState::Stopped => {
            Some(state)
        }
        _ => None,
    }
}

/// What the Board glance says the bot is doing (home.proto `BotStatus`).
/// `waiting`: a row says it waits on the owner, whatever its state.
pub(super) fn status(state: BotState, waiting: bool) -> BotStatus {
    if waiting {
        return BotStatus::Waiting;
    }
    match state {
        BotState::Starting | BotState::Working => BotStatus::Working,
        BotState::Ready => BotStatus::Idle,
        BotState::WaitingForUser | BotState::WaitingForApproval => BotStatus::Waiting,
        BotState::Stopping | BotState::Stopped => BotStatus::Stopped,
        BotState::Crashed | BotState::AuthFailed | BotState::RateLimited => BotStatus::Failed,
    }
}

/// The project's own bots here (stand-ins run elsewhere), in list order.
pub(super) fn here(
    app: &AppState,
    project_id: &str,
    computer: &str,
) -> anyhow::Result<Vec<BotBrief>> {
    let me = app.db.daemon_id()?;
    let lead = app.db.get_project(project_id)?.and_then(|p| p.lead_bot_id);
    let waiting: HashSet<String> = crate::attention::waiting_bots(app, project_id)?
        .into_iter()
        .map(|bot| bot.id)
        .collect();
    let mut briefs: Vec<BotBrief> = app
        .db
        .list_bots(Some(project_id))?
        .iter()
        .filter(|bot| !bot.is_linked())
        .map(|bot| {
            let (state, reason) = app.supervisor.state(&bot.id);
            let status = status(state, waiting.contains(&bot.id));
            let since = match status {
                BotStatus::Waiting => Some(app.overview.waiting_since(&bot.id)),
                _ => app.overview.status_since(&bot.id, status),
            };
            BotBrief {
                daemon_id: me.clone(),
                bot_id: bot.id.clone(),
                name: Db::display_name(bot),
                avatar: bot.avatar.clone(),
                runtime: bot.runtime.as_str().to_string(),
                status: status as i32,
                state: state.as_str().to_string(),
                state_reason: reason,
                computer_name: computer.to_string(),
                lead: lead.as_deref() == Some(bot.id.as_str()),
                since: since.map(timestamp),
            }
        })
        .collect();
    sort(&mut briefs);
    Ok(briefs)
}

/// A linked computer's bots for a row: its part's when it sent any, offline
/// unless it answers now; else its bots' stand-ins here, offline unless it
/// answers (then too old to say: `UNSPECIFIED`).
pub(super) struct Linked<'a> {
    pub peer: &'a bus::Peer,
    pub daemon_id: &'a str,
    pub answering: bool,
    pub part: Option<&'a Part>,
}

/// Every bot of the project across its computers, each once, in list order,
/// at most [`BOTS_SHOWN`]. `stand_ins` are the project's linked bots here;
/// the lead is this computer's, so a stand-in that leads marks its bot.
pub(super) fn merge(
    here: &[BotBrief],
    linked: &[Linked<'_>],
    stand_ins: &[Bot],
    lead: Option<&str>,
) -> Vec<BotBrief> {
    // The bot (by its own computer's ids) this computer's lead stands for.
    let lead_origin: Option<(String, String)> = lead.and_then(|lead| {
        let bot = stand_ins.iter().find(|b| b.id == lead)?;
        let peer = linked
            .iter()
            .find(|l| bot.peer_id.as_deref() == Some(l.peer.id.as_str()))?;
        Some((peer.daemon_id.to_string(), bot.remote_bot_id.clone()?))
    });
    let mut out: Vec<BotBrief> = here.to_vec();
    for l in linked {
        let from_part: Vec<BotBrief> = l
            .part
            .map(|p| p.bots.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|mut brief| {
                if !l.answering {
                    brief.status = BotStatus::Offline as i32;
                    brief.since = None;
                }
                brief
            })
            .collect();
        if !from_part.is_empty() {
            out.extend(from_part);
            continue;
        }
        let status = if l.answering {
            BotStatus::Unspecified
        } else {
            BotStatus::Offline
        };
        out.extend(
            stand_ins
                .iter()
                .filter(|bot| bot.peer_id.as_deref() == Some(l.peer.id.as_str()))
                .map(|bot| BotBrief {
                    daemon_id: l.daemon_id.to_string(),
                    bot_id: bot.remote_bot_id.clone().unwrap_or_default(),
                    name: Db::display_name(bot),
                    avatar: bot.avatar.clone(),
                    runtime: bot.runtime.as_str().to_string(),
                    status: status as i32,
                    state: String::new(),
                    state_reason: String::new(),
                    computer_name: Db::display_peer_name(l.peer),
                    lead: false,
                    since: None,
                }),
        );
    }
    if let Some((daemon_id, bot_id)) = &lead_origin {
        for brief in &mut out {
            brief.lead = &brief.daemon_id == daemon_id && &brief.bot_id == bot_id;
        }
    }
    let mut seen = HashSet::new();
    out.retain(|b| seen.insert((b.daemon_id.clone(), b.bot_id.clone())));
    sort(&mut out);
    out.truncate(BOTS_SHOWN);
    out
}

/// The lead first, then waiting, working, failed and stopped, idle,
/// offline; within a group by name, ignoring case.
fn sort(briefs: &mut [BotBrief]) {
    let group: HashMap<i32, u8> = [
        (BotStatus::Waiting, 1),
        (BotStatus::Working, 2),
        (BotStatus::Failed, 3),
        (BotStatus::Stopped, 3),
        (BotStatus::Idle, 4),
        (BotStatus::Unspecified, 5),
        (BotStatus::Offline, 6),
    ]
    .into_iter()
    .map(|(status, n)| (status as i32, n))
    .collect();
    briefs.sort_by_key(|b| {
        (
            !b.lead,
            group.get(&b.status).copied().unwrap_or(5),
            b.name.to_lowercase(),
        )
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brief(name: &str, status: BotStatus, lead: bool) -> BotBrief {
        BotBrief {
            daemon_id: "d".into(),
            bot_id: name.into(),
            name: name.into(),
            status: status as i32,
            lead,
            ..BotBrief::default()
        }
    }

    #[test]
    fn states_read_as_statuses() {
        assert_eq!(status(BotState::Starting, false), BotStatus::Working);
        assert_eq!(status(BotState::Ready, false), BotStatus::Idle);
        assert_eq!(status(BotState::Ready, true), BotStatus::Waiting);
        assert_eq!(
            status(BotState::WaitingForApproval, false),
            BotStatus::Waiting
        );
        assert_eq!(status(BotState::Stopping, false), BotStatus::Stopped);
        assert_eq!(status(BotState::RateLimited, false), BotStatus::Failed);
    }

    #[test]
    fn the_lead_comes_first_then_by_status_and_name() {
        let mut briefs = vec![
            brief("zed", BotStatus::Idle, false),
            brief("Off", BotStatus::Offline, false),
            brief("amy", BotStatus::Idle, false),
            brief("crash", BotStatus::Failed, false),
            brief("Lead", BotStatus::Idle, true),
            brief("busy", BotStatus::Working, false),
            brief("ask", BotStatus::Waiting, false),
            brief("Halt", BotStatus::Stopped, false),
        ];
        sort(&mut briefs);
        let names: Vec<&str> = briefs.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(
            names,
            ["Lead", "ask", "busy", "crash", "Halt", "amy", "zed", "Off"]
        );
    }

    #[test]
    fn a_long_list_is_cut() {
        let here: Vec<BotBrief> = (0..30)
            .map(|n| brief(&format!("b{n:02}"), BotStatus::Idle, false))
            .collect();
        assert_eq!(merge(&here, &[], &[], None).len(), BOTS_SHOWN);
    }
}

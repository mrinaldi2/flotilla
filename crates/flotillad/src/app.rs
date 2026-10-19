//! Shared daemon state wiring.

use std::sync::Arc;
use std::time::{Instant, SystemTime};

use crate::config::{Config, RuntimeKind};
use crate::db::Db;
use crate::events::Events;
use crate::overrides::{AutoCompactOverride, AUTO_COMPACT_META_KEY};
use crate::runtime::double::DoubleAdapter;
use crate::runtime::mixed::MixedAdapter;
use crate::runtime::RuntimeAdapter;
use crate::secrets::Secrets;
use crate::supervisor::Supervisor;

pub const PROTOCOL_VERSION: u32 = 2;
pub const DAEMON_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Features this daemon serves, advertised in `hello_ok`. Informational: the
/// client uses it to hide a panel it cannot fill, while authorisation stays
/// with the connection's grants.
pub const CAPABILITIES: &[&str] = &[
    "terminal_attach",
    "search",
    "routines",
    "devices",
    "bot_self_management",
    "config",
    "decisions",
    "bot_runtime",
    "chat",
    "permissions",
    "peer_chat",
    "linked_projects",
    "bot_browser",
    "agent_conversations",
    "peer_terminal",
    "peer_browser",
    "browser_input",
    "bot_commands",
    "restart_bot",
    "workers",
    // Commands a bot proposes for the owner to run (H-117 R1).
    "owner_actions",
    "permission_profiles",
    // The projects home (H-128): `projects_overview`, `attention_rows`.
    "projects_overview",
    // Card ids as links (H-203): `item_cards_get`, `item_prefix` on projects.
    "item_cards",
    // Owner threads and pins (H-128 D6, D7): `owner_threads`,
    // `owner_thread_get`, `owner_thread_read`, `project_pin`.
    "owner_threads",
    // A bot's answer to a chat message, posted to its owner thread when
    // the turn didn't `message_owner` (H-192).
    "owner_answers",
    "project_pin",
    // Pull requests and their checks (H-273): `flotilla.pr.v1` reads and pushes,
    // as `bus::contract::pr::CAPABILITIES` names them.
    "pull_requests",
    "checks",
    // The 0.19 board structure requests, named after the fact (0.20.0).
    bus::contract::capability::BOARDS_0_19,
    // The 0.20 Board glance and project setup.
    bus::contract::capability::BOARD_PRESETS,
    bus::contract::capability::PROJECT_DESCRIPTION,
    bus::contract::capability::CREATE_BOT_LEAD,
    bus::contract::capability::PROJECT_REPO_CLONE,
    bus::contract::capability::OVERVIEW_BOTS,
    bus::contract::capability::OVERVIEW_TOP_ATTENTION,
    bus::contract::capability::ATTENTION_BOT_STOPPED,
    bus::contract::capability::ITEM_TYPES_V2,
    bus::contract::capability::COLUMN_RESEARCH,
    // The plan (0.21.0 #43): its board requests and pushes, and its gates
    // in Needs you.
    bus::contract::capability::PLAN_V1,
    bus::contract::capability::ATTENTION_PLAN,
    // Questions for the owner (0.22.0).
    bus::contract::capability::QUESTIONS_V1,
    // Artifacts by kind (0.22.0): `list_artifacts` groups and `used_by`,
    // and a card's linked artifacts among its documents.
    bus::contract::capability::ARTIFACTS_V2,
];

pub struct AppState {
    pub cfg: Config,
    pub db: Db,
    pub events: Events,
    pub secrets: Arc<Secrets>,
    pub supervisor: Supervisor,
    /// Runtime override for `cfg.auto_compact_window`, shared with the
    /// supervisor and persisted in the `meta` table.
    pub auto_compact: AutoCompactOverride,
    /// Live links to peer daemons, shared by the delivery worker (forwarding
    /// to linked bots) and the control plane (pairing and linking).
    pub peers: crate::peer::PeerHub,
    /// Bots' conversations read from their transcripts, for the chat pane.
    pub chat: crate::chat::ChatStore,
    /// Bots working with no task on the board (H-135 G4).
    pub off_board: crate::offboard::OffBoard,
    /// Permission prompts waiting on the owner's answer.
    pub approvals: crate::approval::Approvals,
    /// Bot browsers being watched, one shared stream each.
    pub browsers: crate::browser::streams::BrowserStreams,
    /// The temporary-worker queue's wake-up and placement lock.
    pub workers: crate::workers::Workers,
    /// Board changes, numbered per project, for the clients watching them.
    pub board: crate::kanban::feed::BoardFeed,
    /// The draft kickoffs sent lately, so a double click sends one.
    pub kickoffs: crate::kanban::kickoffs::Kickoffs,
    /// Boards whose home is a peer, as last seen (B9).
    pub board_mirror: crate::kanban::mirror::BoardMirror,
    /// The board keeper's memory of this run.
    pub keeper: crate::kanban::keeper::Keeper,
    /// The projects home's peer answers and liveness (H-128).
    pub overview: crate::overview::Overview,
    /// Bots that used a bearer token, and when (H-044 phase 1).
    pub bearers: crate::bus_auth::BearerLog,
    /// The owner's one-time tickets and the app check behind them (H-044 T4).
    pub owner: crate::bus_auth::owner::Owner,
    /// Paired devices connected now, for Send to phone (H-229).
    pub live_devices: crate::releases::phone::LiveDevices,
    /// The projects with a repository clone running (0.20.0).
    pub repo_clones: crate::repo_clone::Clones,
    pub started_at: Instant,
    /// Wall-clock start, kept alongside the monotonic `started_at` purely so
    /// [`AppState::stale_build`] can compare it against a file mtime.
    started_wall: SystemTime,
}

impl AppState {
    pub fn new(cfg: Config, db: Db) -> anyhow::Result<Arc<Self>> {
        let adapter: Arc<dyn RuntimeAdapter> = match cfg.runtime {
            RuntimeKind::Pty => Arc::new(MixedAdapter::new(&cfg)),
            RuntimeKind::Double => Arc::new(DoubleAdapter),
        };
        Self::with_adapter(cfg, db, adapter)
    }

    /// The daemon's state around a given runtime, for tests that need one
    /// the config cannot name.
    pub fn with_adapter(
        cfg: Config,
        db: Db,
        adapter: Arc<dyn RuntimeAdapter>,
    ) -> anyhow::Result<Arc<Self>> {
        std::fs::create_dir_all(&cfg.home)?;
        crate::safe_git::set_home(&cfg.home);
        std::fs::create_dir_all(cfg.logs_dir())?;
        std::fs::create_dir_all(cfg.projects_dir())?;
        let secrets = Arc::new(Secrets::open_for(&cfg.secrets_dir(), cfg.auth.storage())?);
        let events = Events::new();
        let auto_compact = AutoCompactOverride::default();
        if let Some(stored) = db.get_meta(AUTO_COMPACT_META_KEY)? {
            auto_compact.restore(&stored);
        }
        let supervisor = Supervisor::new(
            adapter,
            cfg.clone(),
            db.clone(),
            events.clone(),
            secrets.clone(),
            auto_compact.clone(),
        );
        let cfg_home = cfg.home.clone();
        let board = crate::kanban::feed::BoardFeed::default().with_history(db.clone());
        let app = Arc::new(Self {
            cfg,
            db,
            events,
            secrets,
            supervisor,
            auto_compact,
            peers: crate::peer::PeerHub::default(),
            chat: crate::chat::ChatStore::default(),
            off_board: Default::default(),
            approvals: crate::approval::Approvals::default(),
            browsers: crate::browser::streams::BrowserStreams::default(),
            workers: crate::workers::Workers::default(),
            board,
            kickoffs: Default::default(),
            board_mirror: crate::kanban::mirror::BoardMirror::default(),
            keeper: Default::default(),
            overview: crate::overview::Overview::default(),
            bearers: crate::bus_auth::BearerLog::default(),
            owner: crate::bus_auth::owner::Owner::new(&cfg_home),
            live_devices: Default::default(),
            repo_clones: Default::default(),
            started_at: Instant::now(),
            started_wall: SystemTime::now(),
        });
        // Every session start picks up what the last one left unfinished.
        let weak = Arc::downgrade(&app);
        app.supervisor.on_start(Box::new(move |bot_id, continues| {
            if let Some(app) = weak.upgrade() {
                crate::resume::pick_up(&app, bot_id, continues);
            }
        }));
        Ok(app)
    }

    /// True when the daemon binary on disk is newer than the running process.
    ///
    /// This is the rebuild-without-restart trap: the old process keeps serving
    /// the old MCP tool list, so a bot is told it cannot do something the
    /// source says it can, and everyone debugs the wrong layer. Surfacing it
    /// in diagnostics turns a silent capability gap into a visible "restart me".
    pub fn stale_build(&self) -> bool {
        std::env::current_exe()
            .and_then(|exe| exe.metadata()?.modified())
            .is_ok_and(|modified| modified > self.started_wall)
    }
}

#[cfg(test)]
mod tests {
    use super::CAPABILITIES;
    use bus::contract::capability;

    #[test]
    fn each_capability_is_advertised_once() {
        let mut seen = std::collections::BTreeSet::new();
        for name in CAPABILITIES {
            assert!(seen.insert(*name), "{name} is listed twice");
        }
    }

    /// The 0.19 board structure is served, so it is advertised; the 0.20
    /// strings join the list as the daemon serves each of them.
    #[test]
    fn the_0_19_board_structure_is_advertised() {
        assert!(CAPABILITIES.contains(&capability::BOARDS_0_19));
        assert!(capability::ALL.contains(&capability::BOARDS_0_19));
    }
}

#[cfg(test)]
mod capability_tests {
    use bus::contract::capability;

    /// Contract strings the daemon doesn't serve yet: each joins the list
    /// once it does. Every 0.21 string is served from #43's step 3, and
    /// every 0.22 one from its daemon steps. The 0.23 ones join from its
    /// daemon steps.
    const NOT_SERVED_YET: &[&str] = &[capability::TEAM_ON_DEMAND_V1, capability::REPO_MIRROR_V1];

    /// The 0.19, 0.20 and 0.21 board and home additions are all served.
    #[test]
    fn every_board_glance_capability_is_served() {
        for capability in capability::ALL {
            if NOT_SERVED_YET.contains(capability) {
                assert!(!super::CAPABILITIES.contains(capability), "{capability}");
                continue;
            }
            assert!(super::CAPABILITIES.contains(capability), "{capability}");
        }
    }
}

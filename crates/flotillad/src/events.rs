//! Internal event bus: server pushes fanned out to all connected clients and
//! consumed internally (routine-run completion, activity previews).

use bus::{Bot, BotState, DecisionComment, DecisionView, Delivery, Message, Project, RoutineRun};
use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Push {
    BotState {
        bot_id: String,
        state: BotState,
        reason: String,
        at: String,
        /// The card the turn starting or ending is about, when the daemon
        /// knows (0.19.0): the card a delivery that started it named, or the
        /// one the bot changed during it.
        #[serde(skip_serializing_if = "Option::is_none")]
        item_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        item_title: Option<String>,
        /// What started the turn, when it says more than the card (0.22.0):
        /// "answer", the owner's answer to the bot's question.
        #[serde(skip_serializing_if = "Option::is_none")]
        cause: Option<&'static str>,
    },
    /// Someone moved, assigned or edited a card, or a move was refused
    /// (0.19.0), for the owner's toasts and phone notices.
    BoardActivity {
        activity: Box<bus::contract::home::BoardActivity>,
        /// The connection that asked for it, which skips it; never sent.
        #[serde(skip)]
        origin: Option<u64>,
    },
    MessageNew {
        message: Message,
    },
    /// A bot's identity changed, or it was created or archived. Bots edit
    /// themselves without asking, so open clients need this to avoid showing a
    /// stale name or avatar.
    BotUpdated {
        bot: Bot,
    },
    /// A project was created, renamed, or archived. Archival is signalled by
    /// `deleted_at` being set, exactly as it is for `BotUpdated`.
    ProjectUpdated {
        project: Project,
    },
    /// A project's worker queue changed: a spawn was queued, placed, waits
    /// for another reason, or finished. Clients refetch `list_workers`.
    WorkersUpdated {
        project_id: String,
    },
    /// This computer's running count or its queue for a slot changed
    /// (0.23.0, `team_on_demand_v1`).
    TeamUpdated {
        team: crate::supervisor::TeamStatus,
    },
    /// A bot's commands changed: one started, ended, was stopped or printed
    /// (0.24.0, `commands_v1`). Clients refetch `list_commands`.
    CommandsChanged {
        bot_id: String,
        project_id: String,
        daemon_id: String,
    },
    /// A `clone_project_repo` clone started (`cloning`), finished (`done`,
    /// with the clone's `path`) or failed (`failed`, with an `error` to
    /// show) (0.20.0).
    RepoClone {
        project_id: String,
        state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// This computer paused or resumed every project for an install
    /// (H-117): the open pause, or none.
    QuiesceUpdate {
        quiesce: Option<Box<crate::db::Quiesce>>,
    },
    /// An owner action was proposed, run, finished or closed (H-117 R1).
    /// Only clients that render owner actions get it.
    OwnerActionUpdate {
        action: Box<serde_json::Value>,
    },
    /// Redacted output of a running owner action, as it comes.
    OwnerActionOutput {
        id: String,
        chunk: String,
    },
    /// A project's meetings or action items changed (H-102). Clients
    /// refetch `meeting_list`, `meeting_get` or `dashboard_get`.
    MeetingEvent {
        project_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        meeting_id: Option<String>,
    },
    /// Rows of the projects home changed (H-128 D4): clients refetch
    /// `projects_overview`. Debounced at 2 s.
    ProjectsOverviewChanged {
        project_ids: Vec<String>,
    },
    /// A bot's owner thread, its read state or its questions changed
    /// (H-128 D6): clients refetch `owner_threads` or the open thread.
    OwnerThreadUpdated {
        bot: bus::contract::home::BotRef,
        project_id: String,
    },
    /// What a release waits on from the owner, or its work card, changed
    /// (H-247): clients refetch that release.
    ReleaseUpdated {
        project_id: String,
        release_id: String,
    },
    /// A project was pinned or unpinned on this computer (H-128 D7).
    ProjectPinned {
        project_id: String,
        pinned: bool,
    },
    /// A bot's sidebar preview line changed. Emitted once the finished turn is
    /// actually readable in the transcript, which lags the `ready` state.
    ActivityUpdate {
        activity: crate::activity::BotActivity,
    },
    DeliveryUpdate {
        delivery: Delivery,
    },
    /// A bot's tool is waiting on the owner's answer.
    PermissionRequest {
        request: crate::approval::PermissionRequest,
    },
    /// A permission prompt stopped waiting: answered, expired or abandoned.
    PermissionResolved {
        request_id: String,
        bot_id: String,
        outcome: crate::approval::Outcome,
    },
    /// Turns of a bot's chat that are new or changed, oldest first.
    ChatTurns {
        bot_id: String,
        turns: Vec<crate::chat::model::ChatTurn>,
    },
    RoutineRunUpdate {
        routine_run: RoutineRun,
    },
    /// A decision was raised, edited, answered, held, published or withdrawn.
    /// Boxed because a decision carries its body, and every other variant
    /// would otherwise pay for the largest one.
    DecisionUpdate {
        decision: Box<DecisionView>,
    },
    DecisionDeleted {
        decision_id: String,
    },
    DecisionCommentNew {
        comment: DecisionComment,
    },
    ApprovalPending {
        bot_id: String,
        detail: String,
        /// The tool waiting for approval, when the notification named one.
        #[serde(skip_serializing_if = "Option::is_none")]
        tool: Option<String>,
    },
    /// The owner sent a paired device an iOS install link (H-229). Only
    /// that device's connections get it.
    InstallOffer {
        device_id: String,
        offer: Box<bus::contract::home::InstallOffer>,
    },
    Notify {
        level: String,
        title: String,
        body: String,
        /// Set when the notice is about a decision, so the client can raise a
        /// native notification that opens the record rather than a toast the
        /// owner has to go looking behind.
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_id: Option<String>,
    },
}

impl Push {
    /// A bot's new state, about no card in particular.
    pub fn bot_state(bot_id: &str, state: BotState, reason: &str) -> Self {
        Push::BotState {
            bot_id: bot_id.to_string(),
            state,
            reason: reason.to_string(),
            at: chrono::Utc::now().to_rfc3339(),
            item_id: None,
            item_title: None,
            cause: None,
        }
    }

    /// A toast: something happened the user should know about but need not act
    /// on. `botmgmt` says it best — "a toast, not a prompt".
    pub fn notice(level: &str, title: impl Into<String>, body: impl Into<String>) -> Self {
        Push::Notify {
            level: level.to_string(),
            title: title.into(),
            body: body.into(),
            decision_id: None,
        }
    }

    /// A notice about a decision. The client may raise this natively, because
    /// an urgent ask or a deadline inside a day is exactly what the owner
    /// cannot afford to find out about later.
    pub fn decision_notice(
        title: impl Into<String>,
        body: impl Into<String>,
        decision_id: &str,
    ) -> Self {
        Push::Notify {
            level: "warn".to_string(),
            title: title.into(),
            body: body.into(),
            decision_id: Some(decision_id.to_string()),
        }
    }
}

/// Internal-only events that are not client pushes.
#[derive(Debug, Clone)]
pub enum Internal {
    /// A bot finished a turn. The Stop hook supplies the transcript used to
    /// correlate routine completion to one exact occurrence.
    BotDone {
        bot_id: String,
        transcript_path: Option<String>,
    },
    /// A bot's runtime asks the owner's permission (Codex approvals).
    RuntimePermission {
        bot_id: String,
        key: u64,
        tool: String,
        input: serde_json::Value,
    },
    /// That request no longer waits on the daemon's answer.
    RuntimePermissionGone { bot_id: String, key: u64 },
}

#[derive(Clone)]
pub struct Events {
    push_tx: broadcast::Sender<Push>,
    internal_tx: broadcast::Sender<Internal>,
}

impl Events {
    pub fn new() -> Self {
        let (push_tx, _) = broadcast::channel(4096);
        let (internal_tx, _) = broadcast::channel(4096);
        Self {
            push_tx,
            internal_tx,
        }
    }

    pub fn push(&self, p: Push) {
        let _ = self.push_tx.send(p);
    }

    pub fn subscribe_push(&self) -> broadcast::Receiver<Push> {
        self.push_tx.subscribe()
    }

    pub fn internal(&self, e: Internal) {
        let _ = self.internal_tx.send(e);
    }

    pub fn subscribe_internal(&self) -> broadcast::Receiver<Internal> {
        self.internal_tx.subscribe()
    }
}

impl Default for Events {
    fn default() -> Self {
        Self::new()
    }
}

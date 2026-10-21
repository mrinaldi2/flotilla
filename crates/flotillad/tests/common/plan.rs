//! A project with planning on (0.21.0): its repository a bare "remote" on
//! disk holding guidelines and documents, a lead bot, and the plan service
//! as the owner or the lead.

use std::path::{Path, PathBuf};

use bus::contract::board as c;
use flotillad::actor::Actor;
use flotillad::db::{MoveTo, NewItem, Write};
use flotillad::kanban::model::{ColumnCategory, ItemType, LinkKind, Priority};
use flotillad::plan::model::Node;
use flotillad::plan::{Outcome, Planned, Planner};

use super::peers::project;
use super::repo::git;
use super::{create_bot, spawn_daemon, TestDaemon, WsClient};

pub struct Plan {
    pub d: TestDaemon,
    pub owner: WsClient,
    pub pid: String,
    pub lead: String,
    pub origin: PathBuf,
    pub _remote: tempfile::TempDir,
}

/// A bare remote whose `main` holds a guideline and an initiative's PRD.
pub fn docs_remote(root: &Path) -> PathBuf {
    let seed = root.join("seed");
    std::fs::create_dir_all(seed.join("docs/guidelines")).expect("dirs");
    std::fs::create_dir_all(seed.join("docs/initiatives/combat")).expect("dirs");
    git(&seed, &["init", "-q", "-b", "main"]);
    std::fs::write(seed.join("docs/guidelines/style.md"), "# Style\n").expect("guideline");
    std::fs::write(
        seed.join("docs/initiatives/combat/prd.md"),
        "# Combat readability\n\nFirst draft.\n",
    )
    .expect("prd");
    git(&seed, &["add", "."]);
    git(&seed, &["commit", "-q", "-m", "docs"]);
    git(root, &["clone", "-q", "--bare", "seed", "origin.git"]);
    root.join("origin.git")
}

/// Commit `text` as `path` on the remote's main; returns the new head.
pub fn push_doc(plan: &Plan, path: &str, text: &str) -> String {
    let work = plan._remote.path().join("work");
    if !work.exists() {
        git(plan._remote.path(), &["clone", "-q", "origin.git", "work"]);
    }
    git(&work, &["pull", "-q", "origin", "main"]);
    let file = work.join(path);
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("dirs");
    std::fs::write(&file, text).expect("write");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "edit"]);
    git(&work, &["push", "-q", "origin", "HEAD:main"]);
    git(&work, &["rev-parse", "HEAD"]).trim().to_string()
}

/// A project named "Game" with its repository, a lead, and planning off.
pub async fn unplanned() -> Plan {
    let remote = tempfile::tempdir().expect("tempdir");
    let origin = docs_remote(remote.path());
    let d = spawn_daemon().await;
    let mut owner = WsClient::connect(&d).await;
    let pid = project(&mut owner, "Game").await;
    let lead = create_bot(&mut owner, &pid, "Lead").await["id"]
        .as_str()
        .expect("bot id")
        .to_string();
    d.app.db.set_project_lead(&pid, Some(&lead)).expect("lead");
    let repo = bus::ProjectRepo {
        url: origin.display().to_string(),
        branch: "main".into(),
    };
    d.app.db.set_project_repo(&pid, Some(&repo)).expect("repo");
    Plan {
        d,
        owner,
        pid,
        lead,
        origin,
        _remote: remote,
    }
}

/// [`unplanned`] with planning turned on, and research closing by itself
/// once its spikes are done, as before the research gate (0.24.0).
pub async fn planned() -> Plan {
    let plan = gated().await;
    assert!(plan
        .owner()
        .set_research_gate(&plan.pid, false)
        .expect("research gate")
        .is_none());
    plan
}

/// [`unplanned`] with planning turned on as the owner turns it on: research
/// waits for the owner's review of its findings (0.24.0).
pub async fn gated() -> Plan {
    let plan = unplanned().await;
    assert!(plan
        .owner()
        .enable(&plan.pid, true)
        .expect("enable")
        .is_none());
    plan
}

impl Plan {
    pub fn owner(&self) -> Planner<'_> {
        Planner {
            app: &self.d.app,
            actor: Actor::User,
        }
    }

    pub fn lead(&self) -> Planner<'_> {
        Planner {
            app: &self.d.app,
            actor: Actor::Bot {
                id: &self.lead,
                project_id: &self.pid,
            },
        }
    }

    pub fn node(&self, id: &str) -> Node {
        self.d
            .app
            .db
            .board_read(|t| t.plan_node(id))
            .expect("read")
            .expect("a node")
    }

    pub fn goal(&self, title: &str) -> Node {
        done(self.owner().create(&c::PlanCreate {
            project_id: self.pid.clone(),
            kind: c::PlanKind::Goal as i32,
            title: title.into(),
            ..Default::default()
        }))
    }

    pub fn initiative(&self, goal: &str, title: &str) -> Node {
        done(self.owner().create(&c::PlanCreate {
            project_id: self.pid.clone(),
            kind: c::PlanKind::Initiative as i32,
            title: title.into(),
            parent_id: Some(goal.into()),
            ..Default::default()
        }))
    }

    /// The owner skips a node's research.
    pub fn skip(&self, node: &Node) -> Node {
        done(self.owner().research_skip(&c::PlanResearchSkip {
            id: node.id.clone(),
            expected_version: node.version,
            reason: "known ground".into(),
        }))
    }

    /// The lead submits the node's gate document with `children`, pushing
    /// the document first when the repository doesn't have it: a gate needs
    /// its document in the repository.
    pub fn submit(&self, node: &Node, path: &str, children: &[&str]) -> Node {
        let origin = self._remote.path().join("origin.git");
        let spec = format!("main:{path}");
        let present = std::process::Command::new("git")
            .args(["cat-file", "-e", &spec])
            .current_dir(&origin)
            .status()
            .is_ok_and(|s| s.success());
        if !present {
            push_doc(self, path, &format!("# {path}\n"));
        }
        done(
            self.lead().submit(&c::PlanSubmit {
                id: node.id.clone(),
                expected_version: node.version,
                doc_path: path.into(),
                children: children
                    .iter()
                    .map(|t| c::PlanChild {
                        title: t.to_string(),
                        summary: Some(format!("{t}, in short.")),
                    })
                    .collect(),
                ..Default::default()
            }),
        )
    }

    /// The owner approves the node's gate, leaving out `leave_out`; returns
    /// the node and what the approval created.
    pub fn approve(&self, node: &Node, leave_out: &[u32]) -> (Node, Vec<String>) {
        let planned = self
            .owner()
            .rule(&c::PlanRule {
                id: node.id.clone(),
                expected_version: node.version,
                approve: true,
                leave_out: leave_out.to_vec(),
                ..Default::default()
            })
            .expect("rule");
        let created = planned.created.clone();
        (done(Ok(planned)), created)
    }

    /// A spike of `node` on its board.
    pub fn spike(&self, node: &Node, title: &str) -> String {
        self.d
            .app
            .db
            .create_item(
                &NewItem {
                    project_id: &self.pid,
                    board_id: node.board_id.as_deref(),
                    item_type: ItemType::Spike,
                    title,
                    description: "",
                    size: None,
                    priority: Priority::P2,
                    labels: &[],
                    parent_id: None,
                    acceptance_criteria: &[],
                    plan_node_id: Some(&node.id),
                },
                &Actor::User,
            )
            .expect("spike")
            .id
    }

    /// Moves the card to its board's Done column, as history would.
    pub fn finish(&self, item_id: &str) {
        let db = &self.d.app.db;
        let item = db.get_item(item_id).expect("read").expect("an item");
        let done = db
            .board_columns(&self.pid)
            .expect("columns")
            .into_iter()
            .find(|c| c.board_id == item.board_id && c.category == ColumnCategory::Done)
            .expect("a done column");
        let to = MoveTo {
            column: &done.key,
            ..MoveTo::default()
        };
        assert!(matches!(
            db.move_item(item_id, item.version, &to, &Actor::User)
                .expect("move"),
            Write::Done(_)
        ));
    }

    /// The lead's kickoff card of `node` whose title starts with `prefix`
    /// ("Research: ", "Write the PRD: ").
    pub fn kickoff(&self, node: &str, prefix: &str) -> Option<String> {
        self.d
            .app
            .db
            .board_read(|t| t.plan_node_cards(node))
            .expect("cards")
            .into_iter()
            .find(|c| c.title.starts_with(prefix))
            .map(|c| c.id)
    }

    /// The lead's research kickoff of `node`, done with a document.
    pub fn research_kickoff_done(&self, node: &str) {
        let card = self
            .kickoff(node, "Research: ")
            .expect("a research kickoff");
        self.finish(&card);
        self.document(&card, "research/kickoff.md");
    }

    /// Links a research document to the card.
    pub fn document(&self, item_id: &str, path: &str) {
        self.d
            .app
            .db
            .add_item_link(item_id, LinkKind::Artifact, path, None, &Actor::User)
            .expect("link");
    }
}

pub fn done(planned: anyhow::Result<Planned>) -> Node {
    match planned.expect("planned").outcome {
        Outcome::Done(node) => *node,
        other => panic!("not done: {other:?}"),
    }
}

/// The codes of a refused change.
pub fn refused(planned: anyhow::Result<Planned>) -> Vec<String> {
    match planned.expect("planned").outcome {
        Outcome::Refused(unmet) => unmet.into_iter().map(|u| u.code).collect(),
        other => panic!("not refused: {other:?}"),
    }
}

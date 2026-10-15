//! Step 8 of the K9 scenario: nothing the project shows its owner or its
//! bots names this repository's own product: its desktop and iOS apps, its
//! daemon, or releases.
//!
//! Kanban's surfaces must say none of it: the board snapshot the apps get,
//! what the bots read from the board, the templates, the labels, the
//! messages the bots got, the board tools and the prompt's board section.
//! The rest of the prompt and the other modules' tools are scanned too,
//! by an ignored test that lists what is left there.

use bus::contract::board as c;
use serde_json::{json, Value};

use super::super::TestDaemon;
use super::K9;
use flotillad::kanban::model::TemplateKind;

/// A preset's name, offered by `board_create`: the built-in flow 0.18
/// boards migrated to. Naming it is not a board of this project using it.
pub const PRESET_NAMES: &[&str] = &["release_gated"];

/// A word step 8 looks for: "release" finds "releases" and the parts of
/// `release_submit`, but "ios" doesn't find "ratios".
fn flagged(part: &str) -> bool {
    let w = part.to_lowercase();
    w == "ios"
        || ["desktop", "daemon", "release"]
            .iter()
            .any(|t| w.starts_with(t))
}

/// Every flagged word in `text` but those `allowed`, with a little of what
/// surrounds it.
pub fn hits(text: &str, allowed: &[&str]) -> Vec<String> {
    let word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let mut out = Vec::new();
    let mut start = None;
    for (i, ch) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        match (word(ch), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                let token = &text[s..i];
                if !allowed.contains(&token) && token.split('_').any(flagged) {
                    let from = text[..s].char_indices().rev().nth(40).map_or(0, |(j, _)| j);
                    let to = text[i..]
                        .char_indices()
                        .nth(40)
                        .map_or(text.len(), |(j, _)| i + j);
                    out.push(text[from..to].replace('\n', " "));
                }
                start = None;
            }
            _ => {}
        }
    }
    out
}

/// The string values in `v`, not its keys: a field named `home_daemon_id`
/// is the wire's, not the project's.
pub fn strings(v: &Value) -> String {
    match v {
        Value::String(s) => format!("{s}\n"),
        Value::Array(items) => items.iter().map(strings).collect(),
        Value::Object(map) => map.values().map(strings).collect(),
        _ => String::new(),
    }
}

/// One surface of the project and what it says.
pub struct Surface {
    pub name: String,
    pub text: String,
    pub allowed: &'static [&'static str],
}

fn surface(name: impl Into<String>, text: impl Into<String>) -> Surface {
    Surface {
        name: name.into(),
        text: text.into(),
        allowed: &[],
    }
}

/// Each surface's hits, for an assertion message; empty when clean.
pub fn report(surfaces: &[Surface]) -> Vec<String> {
    surfaces
        .iter()
        .flat_map(|s| {
            let found = hits(&s.text, s.allowed);
            found.into_iter().map(move |h| format!("{}: …{h}…", s.name))
        })
        .collect()
}

/// The snapshot without the boards the owner archived, as the apps list
/// the project's boards.
fn live(mut s: c::BoardSnapshot) -> c::BoardSnapshot {
    let archived: Vec<String> = s
        .boards
        .iter()
        .filter(|b| b.archived)
        .map(|b| b.id.clone())
        .collect();
    s.boards.retain(|b| !b.archived);
    s.columns.retain(|col| !archived.contains(&col.board_id));
    s.cards.retain(|card| !archived.contains(&card.board_id));
    s
}

fn system_md(d: &TestDaemon, bot_id: &str) -> String {
    let bot = d.app.db.get_bot(bot_id).unwrap().expect("bot");
    let workspace = std::path::Path::new(&bot.workspace_path);
    let root = workspace.parent().expect("root");
    std::fs::read_to_string(root.join("system.md")).expect("system.md")
}

/// The prompt's board section, from its heading to the next.
fn board_section(system_md: &str) -> String {
    let start = system_md
        .find("## Work is on the board")
        .expect("a board section");
    let rest = &system_md[start..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |e| e + 3);
    rest[..end].to_string()
}

/// A tool list's tools, each with its description and its arguments'.
fn descriptions(tools: &Value) -> Vec<(String, String)> {
    let listed = tools["tools"].as_array().into_iter().flatten();
    listed
        .map(|tool| {
            let name = tool["name"].as_str().unwrap_or_default().to_string();
            let about = tool["description"].as_str().unwrap_or_default();
            let text = format!("{name}\n{about}\n{}", schema_text(&tool["inputSchema"]));
            (name, text)
        })
        .collect()
}

/// The descriptions and enum values in a JSON schema.
fn schema_text(schema: &Value) -> String {
    match schema {
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| match (k.as_str(), v) {
                ("description", Value::String(s)) => format!("{s}\n"),
                ("enum", values) => strings(values),
                _ => schema_text(v),
            })
            .collect(),
        Value::Array(items) => items.iter().map(schema_text).collect(),
        _ => String::new(),
    }
}

/// A Kanban tool: the board, item and column tools.
fn kanban_tool(name: &str) -> bool {
    let prefixes = ["board_", "item_", "column_", "automation_"];
    name == "role_set" || prefixes.iter().any(|p| name.starts_with(p))
}

impl K9 {
    /// Kanban's surfaces, as the owner's apps and both bots see them.
    pub async fn kanban_surfaces(&mut self, item_ids: &[&str]) -> Vec<Surface> {
        let snapshot = live(self.desktop_board().await);
        let snapshot = serde_json::to_value(&snapshot).expect("snapshot json");
        let mut out = vec![surface("the desktop's board", strings(&snapshot))];
        for (who, bot) in [("lead", &mut self.lead), ("cook", &mut self.cook)] {
            for tool in ["board_get", "board_list"] {
                let got = bot.call(tool, json!({})).await;
                out.push(surface(format!("{who}'s {tool}"), strings(&got)));
            }
            for id in item_ids {
                let got = bot.call("item_get", json!({ "id": id })).await;
                out.push(surface(format!("{who}'s item_get {id}"), strings(&got)));
            }
        }
        let db = &self.p.mac.app.db;
        for kind in [TemplateKind::ItemType, TemplateKind::MeetingType] {
            for t in db.templates(&self.mac_rb, kind).unwrap() {
                let text = format!("{}\n{}", t.name, strings(&t.body));
                out.push(surface(format!("template {}", t.name), text));
            }
        }
        out.push(surface("the bots' messages", self.told.join("\n")));
        for (who, tools) in self.tool_lists().await {
            for (name, text) in descriptions(&tools).into_iter() {
                if kanban_tool(&name) {
                    out.push(Surface {
                        allowed: PRESET_NAMES,
                        ..surface(format!("{who}'s tool {name}"), text)
                    });
                }
            }
        }
        for (who, md) in self.prompts() {
            out.push(surface(
                format!("{who}'s board section"),
                board_section(&md),
            ));
        }
        out
    }

    /// Everything a bot is told up front and every tool it is offered.
    pub async fn bot_surfaces(&mut self) -> Vec<Surface> {
        let mut out: Vec<Surface> = self
            .prompts()
            .into_iter()
            .map(|(who, md)| surface(format!("{who}'s system.md"), md))
            .collect();
        for (who, tools) in self.tool_lists().await {
            for (name, text) in descriptions(&tools) {
                out.push(Surface {
                    allowed: PRESET_NAMES,
                    ..surface(format!("{who}'s tool {name}"), text)
                });
            }
        }
        out
    }

    fn prompts(&self) -> Vec<(&'static str, String)> {
        vec![
            ("lead", system_md(&self.p.mac, &self.lead_id)),
            ("cook", system_md(&self.p.win, &self.cook_id)),
        ]
    }

    async fn tool_lists(&mut self) -> Vec<(&'static str, Value)> {
        vec![
            ("lead", self.lead.tools().await),
            ("cook", self.cook.tools().await),
        ]
    }
}

//! `flotillad cleanup deleted-projects [--yes]`: what projects deleted
//! before left on disk, previewed, then moved to the trash once the owner
//! says yes. Asked of the running daemon as the owner: the command waits
//! for the owner to allow it in Flotilla, as every owner command does.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

use super::size_text;
use crate::config::Config;
use crate::peer::cli::Daemon;

const USAGE: &str = "usage: flotillad cleanup deleted-projects [--yes]";

pub async fn run(cfg: &Config, args: &[String]) -> anyhow::Result<()> {
    if args.first().map(String::as_str) != Some("deleted-projects") {
        anyhow::bail!("{USAGE}");
    }
    let mut daemon = Daemon::connect(cfg).await?;
    let preview = daemon
        .request(json!({ "type": "deleted_projects_preview" }))
        .await?;
    let preview = &preview["preview"];
    print!("{}", describe(preview));
    let total = preview["total_bytes"].as_u64().unwrap_or(0);
    if total == 0 {
        return Ok(());
    }
    let bin = preview["bin"].as_str().unwrap_or("trash");
    let yes = args.iter().any(|a| a == "--yes") || ask(&format!("Move them to the {bin}? [y/N] "))?;
    if !yes {
        println!("Nothing was moved.");
        return Ok(());
    }
    let done = daemon
        .request(json!({ "type": "deleted_projects_cleanup" }))
        .await?;
    println!("{}", done["summary"].as_str().unwrap_or_default());
    Ok(())
}

fn ask(question: &str) -> anyhow::Result<bool> {
    print!("{question}");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}

/// The preview as the terminal shows it.
pub fn describe(preview: &Value) -> String {
    let mut out = String::new();
    let projects = preview["projects"].as_array().cloned().unwrap_or_default();
    if projects.is_empty() {
        out.push_str("Deleted projects left nothing on this computer.\n");
    }
    for project in &projects {
        let bytes = project["total_bytes"].as_u64().unwrap_or(0);
        out.push_str(&format!(
            "{} ({})\n",
            project["project_name"].as_str().unwrap_or("?"),
            size_text(bytes)
        ));
        for group in project["groups"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "  {}: {}\n",
                group["label"].as_str().unwrap_or("?"),
                size_text(group["bytes"].as_u64().unwrap_or(0))
            ));
            for item in group["items"].as_array().into_iter().flatten() {
                out.push_str(&format!("    {}\n", item["path"].as_str().unwrap_or("?")));
            }
        }
        for refused in project["refused"].as_array().into_iter().flatten() {
            out.push_str(&format!(
                "  left alone: {} ({})\n",
                refused["path"].as_str().unwrap_or("?"),
                refused["reason"].as_str().unwrap_or("?")
            ));
        }
    }
    for linked in preview["linked"].as_array().into_iter().flatten() {
        out.push_str(&format!(
            "{} on {}: {}\n",
            linked["project_name"].as_str().unwrap_or("?"),
            linked["peer_name"].as_str().unwrap_or("?"),
            match linked["state"].as_str() {
                Some("unconfirmed") => "didn't confirm",
                _ => "not told yet",
            }
        ));
    }
    if !projects.is_empty() {
        out.push_str(&format!(
            "In all: {}\n",
            size_text(preview["total_bytes"].as_u64().unwrap_or(0))
        ));
    }
    out
}

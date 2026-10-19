//! The repository tools (0.23.0, `repo_mirror_v1`): `worktree_for` gives a
//! card's worktree; `repo_file`, `repo_diff`, `repo_log` and `repo_grep`
//! read the project's repository without one. Every bot of a project with a
//! repository has them; the descriptions are read by bots, so they name
//! nothing of how the team's tools are built.

use serde_json::{json, Value};

use super::schema::tool;
use crate::app::AppState;
use crate::checkouts::{card, mirror::Mirror, read};

pub(super) const NAMES: &[&str] = &[
    "worktree_for",
    "repo_file",
    "repo_diff",
    "repo_log",
    "repo_grep",
];

pub(super) fn handles(name: &str) -> bool {
    NAMES.contains(&name)
}

pub(super) fn tools() -> Vec<Value> {
    let reference = json!({"type": "string", "description": "A branch, tag or commit; default: the branch new work starts from"});
    let path = json!({"type": "string", "description": "A path in the repository, like src/app"});
    vec![
        tool(
            "worktree_for",
            "The git worktree for a card's code: made on the card's branch the first time it is \
              asked for, then the same one for every bot on the card. Work and commit there, push \
              its branch and pass its path to pr_open. It is removed for you once the card's PR \
              merges, or saved and removed if the card is cancelled. Give `paths` (folders) to \
              check out only those; ask again with more to add some, or with [] for all of it.",
            json!({
                "item_id": {"type": "string", "description": "The card, e.g. H-12"},
                "paths": {"type": "array", "items": {"type": "string"}, "description": "Optional: the folders to check out, e.g. [\"src/app\", \"docs\"]"}
            }),
            vec!["item_id"],
        ),
        tool(
            "repo_file",
            "Read a file of the project's repository at a branch, tag or commit, with no \
              checkout; a folder lists its entries. Long files are cut.",
            json!({"path": path, "ref": reference}),
            vec!["path"],
        ),
        tool(
            "repo_diff",
            "What `head` changed since it branched from `base` (git diff base...head), \
              file by file with the patch, optionally under one path. Long patches are cut.",
            json!({
                "base": {"type": "string", "description": "The branch, tag or commit compared against"},
                "head": reference, "path": path
            }),
            vec!["base"],
        ),
        tool(
            "repo_log",
            "The newest commits of a branch, tag or commit, optionally only those touching a path.",
            json!({
                "ref": reference, "path": path,
                "limit": {"type": "integer", "description": "How many (default 30, max 200)"}
            }),
            vec![],
        ),
        tool(
            "repo_grep",
            "Search the text files of the repository at a branch, tag or commit for an \
              extended regular expression, optionally under one path. At most 300 lines.",
            json!({"pattern": {"type": "string"}, "ref": reference, "path": path}),
            vec!["pattern"],
        ),
    ]
}

fn text<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn required<'a>(args: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    text(args, key).ok_or_else(|| anyhow::anyhow!("'{key}' is required"))
}

pub(super) fn call(
    app: &AppState,
    bot: &bus::Bot,
    name: &str,
    args: &Value,
) -> anyhow::Result<Value> {
    if name == "worktree_for" {
        let paths = match args.get("paths") {
            None | Some(Value::Null) => None,
            Some(Value::Array(list)) => Some(
                list.iter()
                    .map(|p| {
                        p.as_str()
                            .map(str::to_string)
                            .ok_or_else(|| anyhow::anyhow!("'paths' are folder names"))
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?,
            ),
            Some(_) => anyhow::bail!("'paths' is a list of folders"),
        };
        return card::worktree_for(app, bot, required(args, "item_id")?, paths.as_deref());
    }
    let mirror = Mirror::of_project(app, &bot.project_id)?;
    mirror.fresh()?;
    match name {
        "repo_file" => read::file(&mirror, required(args, "path")?, text(args, "ref")),
        "repo_diff" => read::diff(
            &mirror,
            required(args, "base")?,
            text(args, "head"),
            text(args, "path"),
        ),
        "repo_log" => {
            let limit = args
                .get("limit")
                .and_then(Value::as_u64)
                .map(|l| u32::try_from(l).unwrap_or(u32::MAX));
            read::log(&mirror, text(args, "ref"), text(args, "path"), limit)
        }
        "repo_grep" => read::grep(
            &mirror,
            required(args, "pattern")?,
            text(args, "ref"),
            text(args, "path"),
        ),
        other => anyhow::bail!("unknown repository tool: {other}"),
    }
}

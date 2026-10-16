//! Finding what a project left on this computer, each item confined to the
//! root it belongs in. Nothing here moves anything.

use std::path::{Path, PathBuf};

use bus::{Bot, Project};

use super::{history, Group, Item, Leftovers, Refused, Scope};
use crate::activity::claude_projects_dir;
use crate::app::AppState;
use crate::cleanup::discover::{self, BotHere};
use crate::cleanup::scope::Roots;
use crate::trash::confine::{self, Depth};

/// Everything `project` left here: its folder, its bots' history, its git
/// cache, saved work and worktrees.
pub fn collect(app: &AppState, project: &Project) -> Leftovers {
    let mut out = empty(project);
    if refused_in_scratch(app, &mut out) {
        return out;
    }
    let cfg = &app.cfg;
    let bots = own_bots(app, &project.id);
    let others = every_workspace(app);
    for bot in &bots {
        bot_history(app, project, bot, &others, &mut out);
    }
    worktrees(app, project, &bots, &mut out);
    if confine::plain_name(&project.dir_name) {
        let dir = crate::paths::project_dir(cfg, &project.dir_name);
        add(
            &mut out,
            Group::ProjectFiles,
            dir,
            &cfg.projects_dir(),
            Depth::Child,
        );
    } else {
        refuse(
            &mut out,
            &project.dir_name,
            "the project's folder name is not a plain name",
        );
    }
    let cache = format!("{}.git", project.id);
    if confine::plain_name(&cache) && confine::plain_name(&project.id) {
        let repos = cfg.home.join("cache").join("repos");
        add(
            &mut out,
            Group::GitCache,
            repos.join(cache),
            &repos,
            Depth::Child,
        );
        let salvage = cfg.home.join("salvage");
        add(
            &mut out,
            Group::SavedWork,
            salvage.join(&project.id),
            &salvage,
            Depth::Child,
        );
    }
    out
}

/// What `bots` of `project` left here: their folders and their history.
/// For a linked computer told to trash the bots it ran for the project.
pub fn collect_bots(app: &AppState, project: &Project, bots: &[Bot]) -> Leftovers {
    let mut out = empty(project);
    if refused_in_scratch(app, &mut out) {
        return out;
    }
    let others = every_workspace(app);
    let bots_root = crate::paths::project_dir(&app.cfg, &project.dir_name).join("bots");
    for bot in bots {
        if let Some(workspace) = bot_history(app, project, bot, &others, &mut out) {
            if let Some(dir) = workspace.parent() {
                add(
                    &mut out,
                    Group::ProjectFiles,
                    dir.to_path_buf(),
                    &bots_root,
                    Depth::Child,
                );
            }
        }
    }
    out
}

fn empty(project: &Project) -> Leftovers {
    Leftovers {
        project: project.clone(),
        items: Vec::new(),
        refused: Vec::new(),
    }
}

/// Scratch mode touches no folder at all (H-171).
fn refused_in_scratch(app: &AppState, out: &mut Leftovers) -> bool {
    if app.cfg.scratch {
        refuse(
            out,
            &app.cfg.home.display().to_string(),
            "scratch mode touches no folder",
        );
    }
    app.cfg.scratch
}

fn refuse(out: &mut Leftovers, path: &str, reason: &str) {
    out.refused.push(Refused {
        path: path.to_string(),
        reason: reason.to_string(),
    });
}

/// `path` as an item of `group` when it exists and is confined to `root`.
fn add(out: &mut Leftovers, group: Group, path: PathBuf, root: &Path, depth: Depth) {
    if std::fs::symlink_metadata(&path).is_err() {
        return;
    }
    if out.items.iter().any(|i| i.path == path) {
        return;
    }
    match confine::check(&path, root, depth) {
        Ok(()) => out.items.push(Item {
            group,
            bytes: crate::trash::size_of(&path),
            path,
            scope: Scope::Under {
                root: root.to_path_buf(),
                depth,
            },
        }),
        Err(reason) => out.refused.push(Refused {
            path: path.display().to_string(),
            reason,
        }),
    }
}

/// The project's own bots (not stand-ins), archived ones included.
fn own_bots(app: &AppState, project_id: &str) -> Vec<Bot> {
    app.db
        .list_bots_with_archived(Some(project_id))
        .unwrap_or_default()
        .into_iter()
        .filter(|b| !b.is_linked())
        .collect()
}

/// Every workspace this computer knows, for telling one bot's history from
/// another's.
fn every_workspace(app: &AppState) -> Vec<PathBuf> {
    app.db
        .list_bots_with_archived(None)
        .unwrap_or_default()
        .into_iter()
        .map(|b| PathBuf::from(b.workspace_path))
        .filter(|w| w.is_absolute())
        .collect()
}

/// The bot's workspace when it is where its project keeps it: in this
/// home's `projects/<dir>/bots/<bot-dir>/workspace`. A crafted path names
/// someone else's folder, and its history is someone else's too.
fn workspace_of(app: &AppState, project: &Project, bot: &Bot) -> Result<PathBuf, String> {
    let expected =
        crate::paths::bot_dir(&app.cfg, &project.dir_name, &bot.dir_name).join("workspace");
    let workspace = PathBuf::from(&bot.workspace_path);
    let named = confine::plain_name(&bot.dir_name) && confine::plain_name(&project.dir_name);
    if !named || workspace != expected {
        return Err(format!(
            "{}'s workspace is not in its project's folder ({})",
            bot.name, bot.workspace_path
        ));
    }
    crate::paths::own_workspace(&app.cfg, bot)
        .ok_or_else(|| format!("{}'s workspace is outside this home", bot.name))
}

/// The bot's Claude Code and Codex history; returns its workspace when it
/// is the project's to touch.
fn bot_history(
    app: &AppState,
    project: &Project,
    bot: &Bot,
    others: &[PathBuf],
    out: &mut Leftovers,
) -> Option<PathBuf> {
    let workspace = match workspace_of(app, project, bot) {
        Ok(workspace) => workspace,
        Err(reason) => {
            refuse(out, &bot.workspace_path, &reason);
            return None;
        }
    };
    let home = &app.cfg.user_home;
    let claude = claude_projects_dir(home);
    for dir in history::claude_folders(home, &workspace, others) {
        add(out, Group::SessionHistory, dir, &claude, Depth::Child);
    }
    if let Some(bot_dir) = workspace.parent() {
        let codex = history::codex_sessions_dir(home);
        for file in history::codex_files(home, bot_dir) {
            add(out, Group::SessionHistory, file, &codex, Depth::Below);
        }
    }
    Some(workspace)
}

/// The bots' linked worktrees beside the owner's clones (`<repo>-wt-<bot>`
/// under a trusted path). Those inside a workspace go with the project's
/// folder. A tree another project's live bot could own as well is left.
fn worktrees(app: &AppState, project: &Project, bots: &[Bot], out: &mut Leftovers) {
    let Ok(Some(repo)) = app.db.project_repo(&project.id) else {
        return;
    };
    let mut urls = vec![repo.url];
    urls.extend(app.db.extra_repos(&project.id).unwrap_or_default());
    let here: Vec<BotHere> = bots
        .iter()
        .map(|b| {
            let ws = PathBuf::from(&b.workspace_path);
            BotHere {
                id: b.id.clone(),
                name: original_name(b),
                workspace: ws.is_absolute().then_some(ws),
            }
        })
        .collect();
    let strangers: Vec<Bot> = app
        .db
        .list_bots(None)
        .unwrap_or_default()
        .into_iter()
        .filter(|b| b.project_id != project.id && !b.is_linked())
        .collect();
    for url in urls {
        for (found, _) in discover::worktrees(app, &url, &here) {
            let Some(bot) = found.bot else {
                continue;
            };
            if found.path.starts_with(&app.cfg.home) {
                continue;
            }
            if let Some(why) = not_ours(app, &found.path, &bot, &strangers) {
                refuse(out, &found.path.display().to_string(), &why);
                continue;
            }
            if out.items.iter().any(|i| i.path == found.path) {
                continue;
            }
            out.items.push(Item {
                group: Group::Worktrees,
                bytes: crate::trash::size_of(&found.path),
                path: found.path,
                scope: Scope::Worktree {
                    workspace: bot.workspace,
                    bot_name: bot.name,
                    main_clone: found.main_clone,
                },
            });
        }
    }
}

/// The name a bot had before it was archived: its worktrees are named for
/// it, not for the tombstone (`name#suffix`) archiving gave it.
fn original_name(bot: &Bot) -> String {
    match bot.deleted_at {
        Some(_) => bot
            .name
            .rsplit_once('#')
            .map_or_else(|| bot.name.clone(), |(name, _)| name.to_string()),
        None => bot.name.clone(),
    }
}

/// Why a worktree found for `bot` may belong to someone else.
fn not_ours(app: &AppState, path: &Path, bot: &BotHere, strangers: &[Bot]) -> Option<String> {
    if let Err(why) = Roots::of(app, bot.workspace.as_deref(), &bot.name).check(path) {
        return Some(why);
    }
    if let Ok(meta) = crate::worktree::read_meta(path) {
        if meta.bot_id != bot.id {
            return Some("its ownership file names another bot".to_string());
        }
        return None;
    }
    strangers
        .iter()
        .find(|s| {
            let ws = PathBuf::from(&s.workspace_path);
            Roots::of(app, ws.is_absolute().then_some(ws.as_path()), &s.name)
                .check(path)
                .is_ok()
        })
        .map(|s| format!("{} in another project could own it too", s.name))
}

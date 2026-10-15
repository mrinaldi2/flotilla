use std::path::PathBuf;
use std::sync::Arc;

use flotillad::app::AppState;
use flotillad::config::Config;
use flotillad::db::Db;

/// Exit code for a stop that only asks to be restarted, distinct from a crash.
const RECLAIM_EXIT_CODE: i32 = 75;

fn flag_value(args: &[String], flag: &str) -> Option<PathBuf> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
}

/// `flotillad migrate-home [--dry-run | --rollback] [--from <dir>] [--to <dir>]`.
fn migrate_home(cfg: &Config, args: &[String]) -> anyhow::Result<()> {
    use flotillad::migrate_home::{self as migrate, Plan};
    let default = Plan::default_for(cfg);
    let plan = Plan::new(
        flag_value(args, "--from").unwrap_or(default.from),
        flag_value(args, "--to").unwrap_or(default.to),
        default.user_home,
    );
    println!(
        "{}",
        flotillad::config::home_notice(
            &cfg.home,
            flotillad::config::home_override_var().as_deref()
        )
    );
    let mut out = std::io::stdout();
    if args.iter().any(|a| a == "--dry-run") {
        if !migrate::dry_run(&plan, &mut out)? {
            std::process::exit(1);
        }
    } else if args.iter().any(|a| a == "--rollback") {
        migrate::rollback(&plan, &mut out)?;
    } else {
        migrate::run(&plan, &mut out)?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--third-party-notices") {
        print!(
            "{}",
            include_str!("../../../third-party/DAEMON_NOTICES.txt")
        );
        return Ok(());
    }
    // The PreToolUse hook runs once per tool call of every bot: no config,
    // no logging, nothing on stdout but its answer.
    if args.first().map(String::as_str) == Some("guard") {
        std::process::exit(flotillad::bot_permissions::guard::run(&args[1..]));
    }
    // A session's stdio bus server (H-044): stdout is MCP's alone.
    if args.first().map(String::as_str) == Some("bus-proxy") {
        std::process::exit(flotillad::bus_auth::proxy::run(&args[1..]).await);
    }
    // A bot's stdio MCP server, detached from its terminal (H-195 D2b).
    if args.first().map(String::as_str) == Some("mcp-exec") {
        std::process::exit(flotillad::bus_auth::detach::mcp_exec(&args[1..]));
    }
    // Claude Code's hooks (H-044): over the local endpoint, not curl.
    if args.first().map(String::as_str) == Some("hook") {
        std::process::exit(flotillad::bus_auth::hook::run(&args[1..]).await);
    }
    // A check's runner (H-283): the daemon starts it and reads its exit
    // status, the check's result.
    if args.first().map(String::as_str) == Some("check") {
        std::process::exit(flotillad::check_exec::run_cli(&args[1..]));
    }
    // `service install` runs a staged binary with this to check it starts.
    if args.first().map(String::as_str) == Some("--version") {
        println!("flotillad {}", flotillad::app::DAEMON_VERSION);
        println!(
            "identity: {}",
            flotillad::bus_auth::app_identity::identity_line()
        );
        return Ok(());
    }
    let config_path = flag_value(&args, "--config");
    let negotiate_port = args.iter().any(|arg| arg == "--negotiate-port");

    // stderr, not stdout: launchd routes the two to different files, and the
    // desktop app reads the error log when an install never comes up. On
    // stdout these lines were interleaved with CLI output and never seen.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let mut cfg = Config::load(config_path.as_deref())?;
    cfg.scratch |= args.iter().any(|arg| arg == "--scratch")
        || [
            flotillad::brand::env_var_os("SCRATCH"),
            std::env::var_os(flotillad::brand::LEGACY_SCRATCH_MODE_ENV),
        ]
        .iter()
        .any(|v| v.as_deref().is_some_and(|v| v == "1"));
    if cfg.scratch {
        tracing::warn!(
            home = %cfg.home.display(),
            "SCRATCH MODE: no bot starts, no peer links, no routines, deliveries or \
             workers, no bot-folder writes, no git push"
        );
    }
    if let Some(e) = cfg.auth.hold_phase_two(flotillad::bus_auth::HOOKS_OVER_IPC) {
        tracing::error!("{e}");
    }
    let app_identity_known = flotillad::bus_auth::app_identity::APP_IDENTITY_KNOWN;
    if let Some(e) = cfg.auth.hold_for_app_identity(app_identity_known) {
        tracing::error!("{e}");
    }

    // Everything but the service and the migration itself would otherwise
    // start a fresh, empty home next to the one holding the data.
    if !matches!(
        args.first().map(String::as_str),
        Some("service" | "migrate-home")
    ) {
        flotillad::migrate_home::refuse_unmigrated(&cfg)?;
    }

    match args.first().map(String::as_str) {
        Some("migrate-home") => {
            migrate_home(&cfg, &args)?;
            return Ok(());
        }
        Some("backup") => {
            let out = flag_value(&args, "--out").unwrap_or_else(|| {
                flotillad::backup::backups_dir(&cfg).join(format!(
                    "backup-{}",
                    chrono::Utc::now().format("%Y%m%d-%H%M%S")
                ))
            });
            // As stored: a newer binary backs up without migrating first.
            let path = flotillad::backup::backup(&cfg, &out)?;
            println!("backup written to {}", path.display());
            return Ok(());
        }
        Some("restore") => {
            let from = flag_value(&args, "--from")
                .ok_or_else(|| anyhow::anyhow!("restore requires --from <backup dir>"))?;
            let overwrite = args.iter().any(|a| a == "--overwrite");
            flotillad::backup::restore(&cfg, &from, overwrite)?;
            println!("restored into {}", cfg.home.display());
            return Ok(());
        }
        Some("board") => {
            flotillad::backlog_import::cli::run(&cfg, &args[1..]).await?;
            return Ok(());
        }
        Some("release") => {
            flotillad::releases::cli::run(&cfg, &args[1..]).await?;
            return Ok(());
        }
        Some("pr") => {
            flotillad::pr_cli::run(&cfg, &args[1..]).await?;
            return Ok(());
        }
        Some("quiesce") => {
            flotillad::quiesce::cli::run(&cfg, &args[1..]).await?;
            return Ok(());
        }
        Some("peer") => {
            flotillad::peer::cli::run(&cfg, &args[1..]).await?;
            return Ok(());
        }
        Some("service") => {
            let paths =
                flotillad::service::ServicePaths::new(cfg.home.clone(), cfg.user_home.clone());
            // Flags are not otherwise parsed here; `service install --help`
            // used to run the install.
            if args.iter().any(|a| a == "--help" || a == "-h") {
                println!(
                    "usage: flotillad service <install [--binary <path>] [--no-migrate]|uninstall|restart|status [--json]>"
                );
                return Ok(());
            }
            match args.get(1).map(String::as_str) {
                Some("install") => {
                    let source =
                        flag_value(&args, "--binary").map_or_else(std::env::current_exe, Ok)?;
                    println!(
                        "{}",
                        flotillad::config::home_notice(
                            &cfg.home,
                            flotillad::config::home_override_var().as_deref()
                        )
                    );
                    // `--no-migrate` (the app's launch-time repair) reinstalls
                    // the binary only; moving the home needs the user's yes.
                    let pending = flotillad::migrate_home::pending(&cfg)?;
                    if args.iter().any(|a| a == "--no-migrate") {
                        if let Some(plan) = &pending {
                            anyhow::bail!(
                                "{} still has to move to {}; run `flotillad service install` \
                                 without --no-migrate to move it",
                                plan.from.display(),
                                plan.to.display()
                            );
                        }
                    }
                    // Who restarted the service, on record (H-193).
                    let log_dir = flotillad::service_audit::log_dir_for(
                        &paths.log_dir(),
                        pending.as_ref().map(|p| (p.from.as_path(), p.to.as_path())),
                    );
                    flotillad::service_audit::record(&log_dir, &args);
                    flotillad::service::install_and_start(
                        &source,
                        &paths,
                        cfg.port,
                        pending.as_ref(),
                    )?;
                    println!(
                        "flotillad installed to {} and running ({})",
                        paths.bin_path().display(),
                        flotillad::service::SERVICE_LABEL
                    );
                    println!("Logs: {}", paths.log_dir().display());
                }
                Some("uninstall") => {
                    flotillad::service::uninstall(&paths)?;
                    println!(
                        "flotillad service removed; state in {} kept",
                        cfg.home.display()
                    );
                }
                Some("restart") => {
                    anyhow::ensure!(
                        paths.plist_path().exists(),
                        "no flotillad service installed at {}",
                        paths.plist_path().display()
                    );
                    flotillad::service::restart(&paths, cfg.port)?;
                    println!(
                        "flotillad restarted ({})",
                        flotillad::service::SERVICE_LABEL
                    );
                }
                Some("status") => {
                    if args.iter().any(|a| a == "--json") {
                        let home = flotillad::service_report::HomeState {
                            migration_pending: flotillad::migrate_home::pending(&cfg)?.is_some(),
                            migrated: flotillad::migrate_home::completed(&cfg),
                        };
                        let report = flotillad::service::report(&paths, cfg.port, home);
                        println!("{}", serde_json::to_string(&report)?);
                        return Ok(());
                    }
                    if !flotillad::service::status(&paths, cfg.port) {
                        std::process::exit(1);
                    }
                }
                _ => anyhow::bail!("usage: flotillad service <install|uninstall|restart|status>"),
            }
            return Ok(());
        }
        _ => {}
    }

    // Before the port: two daemons on one home run two supervisors against one
    // database, and negotiation means a collision no longer stops the second.
    let _home_lock = flotillad::home::lock(&cfg.home)?;
    #[cfg(unix)]
    flotillad::holders::track_sessions_in(&cfg.home);
    // What bot sessions start, for quiesce to reap (H-117 Q1).
    flotillad::holders::ledger::track_in(&cfg.home);
    flotillad::holders::ledger::spawn_sweeper();
    let policy = flotillad::server::port_policy(&cfg, negotiate_port);
    let bound = flotillad::server::bind(&cfg.bind, cfg.port, policy).await?;
    cfg.port = bound.port();
    if bound.negotiated() {
        tracing::warn!(
            configured_port = cfg.configured_port,
            port = cfg.port,
            "configured port unavailable; app and bot bus are using the active port; \
             remote clients and MCP URL allowlists must use this port"
        );
    }
    let home = cfg.home.clone();
    let configured_port = cfg.configured_port;
    flotillad::home::publish_runtime_port(&home, cfg.port)?;

    // Only a daemon that had to move watches for its port, and only while it
    // has restarts left: a port that keeps flapping is not worth bouncing
    // every bot for.
    let spent = flotillad::home::recent_reclaims(&home);
    let reclaim = bound.negotiated() && spent < flotillad::home::MAX_RECLAIMS;
    if bound.negotiated() {
        if !reclaim {
            tracing::warn!(
                port = configured_port,
                attempts = spent,
                "staying on the negotiated port: too many restarts already spent taking it back"
            );
        }
    } else {
        flotillad::home::clear_reclaims(&home);
    }

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        pid = std::process::id(),
        home = %cfg.home.display(),
        port = cfg.port,
        runtime = ?cfg.runtime,
        "flotillad starting"
    );

    // Opening the database migrates it, one way; back it up first or stop.
    flotillad::backup::before_migrations(&cfg)?;
    let db = Db::open(&cfg.db_path())?;
    if !db.integrity_check()? {
        anyhow::bail!("SQLite integrity check failed; refusing to start");
    }
    let app: Arc<AppState> = AppState::new(cfg, db)?;

    match app.supervisor.adapter().probe() {
        Ok(v) => tracing::info!(runtime_version = %v, "runtime available"),
        Err(e) => {
            tracing::warn!(error = %e, "runtime probe failed; bots cannot start until resolved")
        }
    }

    let stop = flotillad::server::serve(app, bound, reclaim).await?;
    flotillad::home::clear_runtime_port(&home);
    tracing::info!(pid = std::process::id(), "flotillad stopped");

    if stop == flotillad::server::Stop::ReclaimPort {
        let attempts = flotillad::home::record_reclaim(&home);
        tracing::warn!(
            port = configured_port,
            attempts,
            "exiting so the service restarts on the configured port"
        );
        // Non-zero on purpose: the launchd agent is KeepAlive-on-failure, so a
        // clean exit is the one thing that would not bring the daemon back.
        std::process::exit(RECLAIM_EXIT_CODE);
    }
    Ok(())
}

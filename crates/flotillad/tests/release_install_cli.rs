//! `flotillad release install` (B8): the gate is the daemon's, asked over the
//! local endpoint as the calling bot. From outside any bot session, as a
//! terminal or a script, it is refused before anything is fetched.

use crate::common;

use common::*;

#[tokio::test]
async fn release_install_outside_a_bot_session_is_refused() {
    let d = spawn_daemon().await;
    // The local endpoint comes up just after the daemon answers: a CLI
    // started before it found no socket (a flake in CI).
    #[cfg(unix)]
    {
        let socket = d.app.cfg.home.join("run").join("bus.sock");
        for _ in 0..100 {
            if socket.exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        assert!(socket.exists(), "no local endpoint at {}", socket.display());
    }
    let dir = tempfile::tempdir().expect("dir");
    let config = dir.path().join("flotillad.toml");
    std::fs::write(
        &config,
        format!(
            "home = {:?}\nport = {}\n",
            d.app.cfg.home.display().to_string(),
            d.app.cfg.port
        ),
    )
    .expect("config");
    let out = tokio::process::Command::new(env!("CARGO_BIN_EXE_flotillad"))
        .args(["release", "install", "r1", "--config"])
        .arg(&config)
        .output()
        .await
        .expect("run flotillad release install");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{stderr}");
    assert!(stderr.contains("not a bot session"), "{stderr}");
    assert!(stderr.contains("tester's own session"), "{stderr}");
}

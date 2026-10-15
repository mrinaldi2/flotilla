//! How long the helpers wait, on a quiet or a loaded computer (H-162, H-333).

use std::time::Duration;

/// How much longer every wait here runs on a loaded computer:
/// `FLOTILLA_TEST_TIMEOUT_SCALE` (default 1). Waits only bound a failing run.
pub fn timeout_scale() -> u32 {
    std::env::var("FLOTILLA_TEST_TIMEOUT_SCALE")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|n| *n > 0)
        .unwrap_or(1)
}

/// How long a test waits for a frame it expects. 20 s, not 5: on a loaded
/// computer a test binary can sit minutes behind others, and a frame that
/// is coming is never worth failing for (H-333).
pub fn frame_wait() -> Duration {
    Duration::from_secs(20) * timeout_scale()
}

/// How long a test waits for a frame while it sets up: the hello, projects
/// and bots. The first run after a build is cold, and every test binary
/// starts its daemons at once, so a bot's workspace can take far longer to
/// provision than [`frame_wait`] on a slow disk or under a virus scan.
/// `FLOTILLA_TEST_SETUP_WAIT_SECS` overrides it.
pub fn setup_wait() -> Duration {
    std::env::var("FLOTILLA_TEST_SETUP_WAIT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .map_or(
            Duration::from_secs(60) * timeout_scale(),
            Duration::from_secs,
        )
}

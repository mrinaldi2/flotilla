use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{default_toolchain, derive, which, Host};

/// A computer with `dirs` as its real folders and `files` as its files.
struct Fake {
    env: BTreeMap<&'static str, OsString>,
    dirs: BTreeSet<PathBuf>,
    files: BTreeMap<PathBuf, String>,
}

impl Fake {
    fn new(path: &str) -> Self {
        Self {
            env: [("PATH", OsString::from(path))].into(),
            dirs: BTreeSet::new(),
            files: BTreeMap::new(),
        }
    }

    fn derive(&self, home: &str) -> Vec<PathBuf> {
        let env = |k: &str| self.env.get(k).cloned();
        let is_dir = |p: &Path| self.dirs.contains(p);
        let read = |p: &Path| self.files.get(p).cloned();
        let host = Host {
            env: &env,
            home: Some(PathBuf::from(home)),
            is_dir: &is_dir,
            read: &read,
        };
        std::env::split_paths(&derive(&host)).collect()
    }
}

#[cfg(not(windows))]
mod unix {
    use super::*;

    const RUSTUP_PROXIES: &str = "/opt/homebrew/opt/rustup/bin";
    const TOOLCHAIN: &str = "/Users/m/.rustup/toolchains/stable-aarch64-apple-darwin/bin";

    /// The Mac as launchd starts the daemon (H-318): a bare PATH, rustup
    /// keg-only, pnpm in /opt/homebrew/bin.
    fn mac() -> Fake {
        let mut f = Fake::new("/usr/bin:/bin:/usr/sbin:/sbin");
        f.dirs.extend(
            [RUSTUP_PROXIES, TOOLCHAIN, "/opt/homebrew/bin"]
                .into_iter()
                .map(PathBuf::from),
        );
        f.files.insert(
            "/Users/m/.rustup/settings.toml".into(),
            "version = \"12\"\ndefault_toolchain = \"stable-aarch64-apple-darwin\"\n".into(),
        );
        f
    }

    fn strs(dirs: &[PathBuf]) -> Vec<&str> {
        dirs.iter().map(|d| d.to_str().unwrap()).collect()
    }

    #[test]
    fn the_daemons_path_comes_first_then_what_exists_once_each() {
        let mut f = mac();
        f.env
            .insert("PATH", "/opt/homebrew/bin:/usr/bin:/bin".into());
        let got = f.derive("/Users/m");
        assert_eq!(
            strs(&got),
            [
                "/opt/homebrew/bin",
                "/usr/bin",
                "/bin",
                RUSTUP_PROXIES,
                TOOLCHAIN
            ],
            "the owner's order kept; /opt/homebrew/bin not added twice"
        );
    }

    #[test]
    fn homebrew_bin_is_added_when_launchds_path_lacks_it() {
        let got = mac().derive("/Users/m");
        assert_eq!(
            strs(&got),
            [
                "/usr/bin",
                "/bin",
                "/usr/sbin",
                "/sbin",
                RUSTUP_PROXIES,
                TOOLCHAIN,
                "/opt/homebrew/bin"
            ]
        );
    }

    #[test]
    fn rustup_proxies_come_before_the_toolchain() {
        let got = mac().derive("/Users/m");
        let at = |d: &str| got.iter().position(|g| g == Path::new(d)).unwrap();
        assert!(at(RUSTUP_PROXIES) < at(TOOLCHAIN));
    }

    #[test]
    fn a_missing_candidate_is_skipped_and_a_symlinked_one_refused() {
        let mut f = Fake::new("/usr/bin");
        // ~/.cargo/bin isn't there; the rest of the candidates aren't folders.
        f.dirs.insert("/Users/m/.volta/bin".into());
        assert_eq!(
            strs(&f.derive("/Users/m")),
            ["/usr/bin", "/Users/m/.volta/bin"]
        );

        // The real check: a symlinked folder isn't one.
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(super::super::real_dir(&real));
        assert!(!super::super::real_dir(&link));
        assert!(!super::super::real_dir(&dir.path().join("missing")));
    }

    #[test]
    fn cargo_home_and_pnpm_home_win_over_the_defaults() {
        let mut f = Fake::new("/usr/bin");
        f.env.insert("CARGO_HOME", "/opt/cargo".into());
        f.env.insert("PNPM_HOME", "/opt/pnpm".into());
        f.dirs.extend(
            ["/opt/cargo/bin", "/opt/pnpm", "/Users/m/.cargo/bin"]
                .into_iter()
                .map(PathBuf::from),
        );
        assert_eq!(
            strs(&f.derive("/Users/m")),
            ["/usr/bin", "/opt/cargo/bin", "/opt/pnpm"]
        );
    }

    #[test]
    fn a_short_default_toolchain_gets_this_computers_triple() {
        let mut f = Fake::new("/usr/bin");
        f.files.insert(
            "/h/.rustup/settings.toml".into(),
            "default_toolchain = \"stable\"\n".into(),
        );
        let full = format!(
            "/h/.rustup/toolchains/stable-{}/bin",
            super::super::triple()
        );
        f.dirs.insert(full.clone().into());
        assert_eq!(strs(&f.derive("/h")), ["/usr/bin", full.as_str()]);
    }

    #[test]
    fn which_finds_a_tool_on_the_given_path_only() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        std::fs::write(bin.join("cargo"), "").unwrap();
        let path = std::env::join_paths([Path::new("/nonexistent"), &bin]).unwrap();
        assert_eq!(which("cargo", &path), Some(bin.join("cargo")));
        assert_eq!(which("pnpm", &path), None);
    }
}

#[test]
fn the_settings_default_toolchain_line_is_read() {
    let settings =
        "version = \"12\"\ndefault_toolchain = \"stable-aarch64-apple-darwin\"\n\n[overrides]\n";
    assert_eq!(
        default_toolchain(settings).as_deref(),
        Some("stable-aarch64-apple-darwin")
    );
    assert_eq!(
        default_toolchain("default_toolchain = \"1.98.1-aarch64-apple-darwin\"").as_deref(),
        Some("1.98.1-aarch64-apple-darwin")
    );
    assert_eq!(default_toolchain("version = \"12\"\n"), None);
    assert_eq!(default_toolchain("default_toolchain = \"../../etc\""), None);
    assert_eq!(default_toolchain("default_toolchain = \"\""), None);
}

#[cfg(windows)]
mod windows {
    use super::*;

    #[test]
    fn windows_candidates_come_from_the_users_folders() {
        let mut f = Fake::new(r"C:\Windows\system32");
        f.env
            .insert("LOCALAPPDATA", r"C:\Users\m\AppData\Local".into());
        f.env
            .insert("APPDATA", r"C:\Users\m\AppData\Roaming".into());
        f.env.insert("ProgramFiles", r"C:\Program Files".into());
        f.dirs.extend(
            [
                r"C:\Users\m\.cargo\bin",
                r"C:\Users\m\AppData\Local\pnpm",
                r"C:\Users\m\AppData\Roaming\npm",
                r"C:\Program Files\nodejs",
            ]
            .into_iter()
            .map(PathBuf::from),
        );
        let got = f.derive(r"C:\Users\m");
        assert_eq!(
            got,
            [
                r"C:\Windows\system32",
                r"C:\Users\m\.cargo\bin",
                r"C:\Users\m\AppData\Local\pnpm",
                r"C:\Users\m\AppData\Roaming\npm",
                r"C:\Program Files\nodejs",
            ]
            .map(PathBuf::from)
        );
    }

    #[test]
    fn which_resolves_exe_then_cmd() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pnpm.cmd"), "").unwrap();
        std::fs::write(dir.path().join("cargo.exe"), "").unwrap();
        let path = dir.path().as_os_str();
        assert_eq!(which("pnpm", path), Some(dir.path().join("pnpm.cmd")));
        assert_eq!(which("cargo", path), Some(dir.path().join("cargo.exe")));
        assert_eq!(which("node", path), None);
    }
}

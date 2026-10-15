use super::*;

const LIMITS: Limits = Limits {
    jobs_per_machine: 2,
    heavy_per_machine: 1,
    disk_floor_gb: 20,
};

fn machine(name: &str, here: bool, tools: &[(&str, &str)]) -> Machine {
    Machine {
        name: name.to_string(),
        tools: tools
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        open_jobs: 0,
        open_heavy: 0,
        here,
        online: true,
    }
}

/// The team as it is: the Mac has everything, the iMac no Node, win-pc is
/// Windows with cargo.
fn team() -> Vec<Machine> {
    vec![
        machine(
            "imac",
            false,
            &[
                ("cargo", "1.90.0"),
                ("python3", "3.12.1"),
                ("xcodebuild", "16.0"),
                (OS, "macos"),
                (DISK_FREE_GB, "300"),
            ],
        ),
        machine(
            "mac",
            true,
            &[
                ("cargo", "1.90.0"),
                ("node", "24.1.0"),
                ("pnpm", "10.1.0"),
                ("docker", "27.1.1"),
                ("python3", "3.12.1"),
                ("mkdocs", "1.6.0"),
                (OS, "macos"),
                (DISK_FREE_GB, "90"),
            ],
        ),
        machine(
            "win-pc",
            false,
            &[("cargo", "1.90.0"), (OS, "windows"), (DISK_FREE_GB, "200")],
        ),
    ]
}

fn needs(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn to(name: &str) -> Route {
    Route::To(name.to_string())
}

/// AC1: rust goes to the Mac, windows to win-pc, and the Node checks never
/// to the iMac.
#[test]
fn each_check_goes_where_its_tools_are() {
    let team = team();
    assert_eq!(
        route(&needs(&["cargo"]), None, false, &team, LIMITS),
        to("mac")
    );
    assert_eq!(
        route(&needs(&["cargo"]), Some("windows"), false, &team, LIMITS),
        to("win-pc")
    );
    for n in [
        needs(&["node", "pnpm"]),
        needs(&["node", "docker"]),
        needs(&["python3", "mkdocs"]),
    ] {
        assert_eq!(route(&n, None, false, &team, LIMITS), to("mac"), "{n:?}");
    }
    // Even with the Mac full, a Node check waits rather than go to the iMac.
    let mut busy = team.clone();
    busy[1].open_jobs = 2;
    let waits = route(&needs(&["node", "pnpm"]), None, false, &busy, LIMITS);
    assert!(
        matches!(&waits, Route::Wait(w) if w.contains("free slot")),
        "{waits:?}"
    );
    // rust spills over to the least busy computer with cargo.
    assert_eq!(
        route(&needs(&["cargo"]), None, false, &busy, LIMITS),
        to("imac")
    );
}

/// AC2: no computer runs more than its cap; one under the disk floor gets
/// nothing.
#[test]
fn the_cap_and_the_disk_floor_hold_a_check_back() {
    let mut team = team();
    team[2].open_jobs = 2;
    let full = route(&needs(&["cargo"]), Some("windows"), false, &team, LIMITS);
    assert!(
        matches!(&full, Route::Wait(w) if w.contains("2 check jobs")),
        "{full:?}"
    );
    team[2].open_jobs = 1;
    assert_eq!(
        route(&needs(&["cargo"]), Some("windows"), false, &team, LIMITS),
        to("win-pc")
    );
    team[1].tools.insert(DISK_FREE_GB.into(), "12".into());
    let low = route(&needs(&["node"]), None, false, &team, LIMITS);
    assert!(
        matches!(&low, Route::Wait(w) if w.contains("under 20 GB")),
        "{low:?}"
    );
}

#[test]
fn an_offline_computer_makes_the_check_wait() {
    let mut team = team();
    team[2].online = false;
    let offline = route(&needs(&["cargo"]), Some("windows"), false, &team, LIMITS);
    assert_eq!(
        offline,
        Route::Wait("waiting for win-pc to come online".into())
    );
}

/// H-318 AC1: no reported computer has the tool, online or not: unroutable,
/// naming what is missing and what each computer has.
#[test]
fn a_tool_no_computer_reported_is_unroutable() {
    let mut team = team();
    team[1].online = false;
    let none = route(&needs(&["swift"]), None, false, &team, LIMITS);
    let Route::Unroutable(why) = &none else {
        panic!("{none:?}");
    };
    assert!(
        why.starts_with("no computer can run this: needs swift; "),
        "{why}"
    );
    assert!(
        why.contains("imac (macos) has cargo, python3, xcodebuild"),
        "{why}"
    );
    assert!(why.contains("win-pc (windows) has cargo"), "{why}");
    assert!(!why.contains(DISK_FREE_GB), "{why}");
    // Only offline, the Mac's pnpm is still a wait.
    assert_eq!(
        route(&needs(&["pnpm"]), None, false, &team, LIMITS),
        Route::Wait("waiting for mac to come online".into())
    );
    let nobody = route(&needs(&["cargo"]), None, false, &[], LIMITS);
    assert_eq!(
        nobody,
        Route::Unroutable(
            "no computer can run this: needs cargo; no computer has reported its tools yet".into()
        )
    );
}

/// H-318 AC1: a Windows check where only Macs report is unroutable.
#[test]
fn a_windows_check_with_only_macs_reporting_is_unroutable() {
    let macs: Vec<Machine> = team().into_iter().take(2).collect();
    let why = route(&needs(&["cargo"]), Some("windows"), false, &macs, LIMITS);
    assert!(
        matches!(&why, Route::Unroutable(w)
            if w.starts_with("no computer can run this: needs cargo on a windows computer; ")),
        "{why:?}"
    );
    let bare = route(&[], Some("windows"), false, &macs, LIMITS);
    assert!(
        matches!(&bare, Route::Unroutable(w) if w.contains("needs a windows computer")),
        "{bare:?}"
    );
}

#[test]
fn a_machine_is_named_by_its_name_or_its_os() {
    let team = team();
    assert!(names("IMAC", &team[0]));
    assert!(names("mac", &team[0]));
    assert!(names("windows", &team[2]));
    assert!(!names("windows", &team[1]));
}

#[test]
fn the_program_a_check_runs_is_needed_too() {
    assert_eq!(
        needs_of(&needs(&["cargo"]), "node scripts/verify.mjs --only rust"),
        needs(&["cargo", "node"])
    );
    assert_eq!(
        needs_of(&needs(&["node", "docker"]), "CI=true scripts/vr-ci.sh"),
        needs(&["node", "docker"])
    );
    assert_eq!(
        needs_of(&needs(&["cargo"]), "cargo test --workspace -j 2"),
        needs(&["cargo"])
    );
    // The real rust check then never lands on the iMac, even when the Mac is full.
    let mut team = team();
    team[1].open_jobs = 2;
    let rust = needs_of(&needs(&["cargo"]), "node scripts/verify.mjs --only rust");
    assert!(matches!(
        route(&rust, None, false, &team, LIMITS),
        Route::Wait(_)
    ));
}

/// H-334: a computer runs one heavy check at a time. A second waits for
/// its slot (an ordinary wait, never unroutable) while a light check still
/// gets the computer's other slot, and an idle computer with the tools
/// takes the heavy one.
#[test]
fn a_heavy_check_waits_for_the_computers_heavy_slot() {
    let mut team = team();
    team[2].open_jobs = 1;
    team[2].open_heavy = 1;
    let rust = needs(&["cargo"]);
    let waits = route(&rust, Some("windows"), true, &team, LIMITS);
    assert!(
        matches!(&waits, Route::Wait(w) if w.contains("another heavy check") && w.contains("win-pc")),
        "{waits:?}"
    );
    // S2: it names only the computers this check could use (its pin).
    assert!(
        matches!(&waits, Route::Wait(w) if !w.contains("mac")),
        "{waits:?}"
    );
    // Its other slot still takes a check that isn't heavy.
    assert_eq!(
        route(&rust, Some("windows"), false, &team, LIMITS),
        to("win-pc")
    );
    // Unpinned, it goes to a computer with cargo and a free heavy slot.
    assert_eq!(route(&rust, None, true, &team, LIMITS), to("mac"));
}

/// H-334: heavy is what `checks.toml` says, else whether it needs cargo.
#[test]
fn heavy_means_cargo_unless_the_check_says() {
    assert!(is_heavy(&needs(&["cargo", "node"]), None));
    assert!(!is_heavy(&needs(&["node", "pnpm"]), None));
    assert!(!is_heavy(&needs(&["cargo"]), Some(false)));
    assert!(is_heavy(&needs(&["docker"]), Some(true)));
    // `run = "cargo test"` with no `needs` counts by its program.
    assert!(is_heavy(&needs_of(&[], "cargo test --workspace"), None));
}

# Addendum: a macOS computer per bot, and what to do this week

Research and design only. No product code was changed, nothing was pushed, and no VM
was created or image downloaded. Written 2026-10-14 as an addendum to
[`per-bot-vms.md`](per-bot-vms.md) (the "report" below). Repository references are to
`gravity-wt-k9` (branch `k9-acceptance`, current with `release-0.19.0-kanban-rework`),
relative to `crates/hermesd/src/` unless they start with another top-level folder.

## 0. Summary

- **The owner's actual problem.** Two bots building two iPhone apps, or two Unity
  games, at once on the laptop. They fight over CPU, GPU, simulators, DerivedData and
  Unity editors, and Simulator, Xcode and Unity windows keep taking focus. The report's
  Ubuntu-first MVP doesn't help with any of this, because iOS work and Unity's iOS
  builds need macOS.
- **The laptop can't host macOS VMs today.** It is a fanless **MacBook Air M4** with 4
  performance and 6 efficiency cores, **24 GB RAM** and **16 GiB free** on a 460 GB
  disk (97 % used), all measured. One tart guest with Xcode already uses about 33-80 GB
  on disk and wants 8 GB of RAM. Two of them don't fit, and a fanless machine throttles
  under sustained builds.
- **The iMac is Apple silicon, and nearly idle.** It is already a linked computer
  (`imac`). It reports **Xcode 27.0** and **274 GB free**. Apple's Xcode 27 release
  notes say "Xcode 27 will only install and run on Apple silicon Macs", so the report's
  "Intel iMac" assumption is wrong.
- **Recommendation, in three steps.**
  1. **This week, with no new code:** move the iOS and Unity bots to the iMac, through
     the existing linked-computer feature (`create_bot` with `machine`, or
     `peer_id`). Make every build headless with per-bot directories through bot
     instructions and `.hermes/checks.toml`. That frees the laptop the same day.
  2. **About 3-5 days of code, still with no VMs:**
     - per-bot build directories in each session's environment;
     - guard rules that stop GUI launches;
     - `taskpolicy -c utility` around every bot session;
     - a Unity tool probe;
     - a machine-wide "heavy build" slot that bots' own builds also take, not only PR
       checks.
  3. **The macOS computer per bot (about 4-5 weeks):** a **`tart` provider** driven
     through tart's CLI, at most two macOS guests per Mac. Each guest runs a hermesd
     in computer mode, paired as a `computer` peer: the report's Option B, unchanged.
     The guest's screen goes through tart `--vnc-experimental` into the report's
     VNC-to-frames bridge, so nothing opens on the host. Host the guests on the iMac or
     a Mac mini, not the Air.
  4. Then Ubuntu containers for web and backend bots (the report's MVP), then Windows.
- **The hard limits are real.**
  - The framework refuses a third running macOS guest (`VZErrorDomain` code 6), and
    the macOS licence allows two extra instances per Mac.
  - Metal in a guest is paravirtualized, and close to native in benchmarks. Unity's
    editor and Play Mode in a guest are **untested** in public; they need a one-hour
    test before any commitment.
  - A physical iPhone over USB into a guest is new in macOS 27 and unproven for Xcode.
    Device installs stay on the host.

---

## 1. Assumptions and measured facts

These assumptions stand until the owner corrects them:

1. **The pain is on the laptop, while the owner studies.** Bots may keep running
   elsewhere at full speed. The goal is "the laptop is mine", not "every bot is
   isolated".
2. **The bots are Claude Code or Codex sessions run by hermesd** as the owner's user,
   as today. iOS bots run `xcodebuild`/`simctl`; Unity bots run the editor or the
   `unity` CLI.
3. **Unity is Unity Personal**, signed in through Hub. That is measured on this Mac by
   the research agent: `~/Library/Unity/licenses/UnityEntitlementLicense.xml`,
   `unity license status`.
4. **Physical-iPhone installs** (the release flow's `iphone` target, `releases/machines.rs:31`)
   stay on a real Mac. They don't need to move into a guest.
5. **Buying hardware is acceptable if it is clearly worth it**, and it is never required
   for step 1.

**Measured on this Mac** (cheap read-only commands):

| Fact | Value | How |
|---|---|---|
| Model | MacBook Air, `Mac16,12`, **fanless** | `system_profiler SPHardwareDataType` |
| Chip | Apple M4, **4 P + 6 E cores**, 10-core GPU, Metal 4 | `sysctl hw.perflevel*`, `SPDisplaysDataType` |
| RAM | **24 GB** | `hw.memsize` |
| Disk | 460 GiB, **16 GiB free (97 % used)** | `df -h /System/Volumes/Data` |
| OS / Xcode | macOS 27.0 (26A428), Xcode 27.0 (27A266a) | `sw_vers`, `xcodebuild -version` |
| Simulator GUI | Xcode 27 replaced Simulator.app with **Device Hub** (`com.apple.dt.Devices`) | research agent, `ls Xcode.app/Contents/Applications`; Xcode 27 release notes |
| Simulators | iOS 27.0 runtime plus watchOS 26.5/27.0. Bots already create their own devices: `Gravity-iOSQA`, `Gravity-iOSDev`, `Gravity-K7`, two iPads. One `iPhone 18 Pro` is booted | `xcrun simctl list` |
| Disk hogs | CoreSimulator **18 GB**, Unity 6000.6.0f1 editor **20 GB**, DerivedData 2.5 GB (one `TheHermes-*` folder per worktree path, plus a shared `ModuleCache.noindex`) | `du -sh` |
| A Unity project | `~/Developer/remi/remi`: Assets **55 MB**, Library **3.3 GB** | `du -sh` |
| Unity tooling | Hub, editor 6000.6.0f1 with iOS support, the new `unity` CLI 1.0.0-beta.8 in `~/.unity/bin` | research agent, local |
| VM tools | lima 2.2.0 (its `macos` template defaults to a macOS 26.5.2 IPSW), colima 0.10.3; **no tart, no UTM** | `which`, `limactl create --list-templates` |
| PID through wrappers | `taskpolicy -c utility` and `nice -n 10` **exec in place**: the child keeps the spawned PID, so bus identity by session-root PID survives a wrapper | `( taskpolicy -c utility sh -c 'echo $$' & echo $! )` |

**The owner's linked computers**, from this daemon's `machine_tool` and `peer` tables
(read-only) and `tailscale status`:

| Machine | OS | Tools it reports | Free disk | Notes |
|---|---|---|---|---|
| `mac` (this Air) | macOS | cargo, node, pnpm, docker, python3, xcodebuild 27.0 | 17 GB | `manuels-macbook-air` |
| **`imac`** | macOS | cargo, python3, **xcodebuild 27.0**; no node, no docker | **274 GB** | Peer `ws://100.105.74.97:49777/peer`. **Apple silicon**, because Xcode 27 only runs there. Chip, RAM, Unity: unknown (Q1) |
| `win-pc` | Windows | cargo, node, docker, python3 | 175 GB | Peer `ws://100.66.104.52:49777/peer` |

Sources for the Xcode 27 claim:
- Apple's release notes, under "Intel Deprecation": "Xcode 27 will only install and run
  on Apple silicon Macs", and "Xcode 27 requires a Mac running macOS Tahoe 26.6 or
  later". Read from
  https://developer.apple.com/tutorials/data/documentation/xcode-release-notes/xcode-27-release-notes.json,
  the data behind https://developer.apple.com/documentation/xcode-release-notes/xcode-27-release-notes.
- The system-requirements page lists macOS Tahoe 26.6 or later, without the Apple
  silicon note: https://developer.apple.com/xcode/system-requirements/.

---

## 2. Why bots fight today, and where Gravity already touches it

**How a bot builds now.**
- A bot's PTY session (`runtime/pty.rs:52`, portable-pty `CommandBuilder`) runs
  `claude`. Every `xcodebuild`, `simctl`, `Unity` or `open` it starts is a child of
  that session, running as the owner's user in the owner's GUI session.
- Anything that opens a window therefore opens it **on the owner's screen**.
- Two bots share one CoreSimulatorService, the same `~/Library/Developer`, one SwiftPM
  cache, and all 10 cores.

**What Gravity already does about it:**

| Mechanism | Where | Today |
|---|---|---|
| Session environment | `supervisor/lifecycle.rs:125-146` builds `env`. `workers/scratch.rs:37` `bot_env` adds `with_bot_target` | Per-bot `CARGO_TARGET_DIR` (`workers/target.rs`, H-029). **The precedent for per-bot DerivedData and Library.** |
| The bot's generated `--settings` | `bot_permissions/settings.rs` (allows `Bash(xcodebuild *)`, `Bash(xcrun simctl *)`) | Claude Code settings may carry `env`. Per `workers/target.rs`, a bot's own settings `env` wins over the session's |
| Guard hook (PreToolUse) | `bot_permissions/guard.rs`, `guard/commands.rs:206,348` | Denies `simctl … booted/all` and `pkill/killall`. Treats `colima start`/`limactl start` as VM starts that block release installs (`guard/commands.rs:342`) |
| Prompt guidance | `paths/prompt_sections.rs:246` `shared_computer()` | "a simulator … you created and address by its UDID", own worktrees, the `df -h` 20 GB rule, "stop any VM you started" |
| PR checks | `.hermes/checks.toml` (schema `prs/policy.rs:98`: `name, run, needs, machine, required, paths, heavy`), run by `check_exec.rs` in a fresh checkout with a cleared environment (`KEPT_ENV`) | Routed by probed tools (`machine_tools.rs:20`: node, pnpm, cargo, xcodebuild, python3, mkdocs, docker; **no Unity**) and `machine` (`prs/check_route.rs`). `heavy = true` takes the computer's single heavy slot (H-334, `[checks] heavy_per_machine`, table `check_heavy`). **Covers PR checks only, not a bot's own builds in its session.** |
| Placement on another computer | `create_bot{peer_id}` (control plane), MCP `create_bot{machine}`, `spawn_worker{machine}` (`docs/peer-bots.md`, `docs/workers.md`) | Works today. A bot created on the iMac runs, builds and opens its windows **there** |
| Cleanup | `cleanup/remove.rs:20` `BUILD_OUTPUT` includes `DerivedData` | Retired workers' build output is removed |
| Launchd service | `service.rs:111-133` | No `ProcessType`/`Nice`. Bots run at the owner's normal priority |

---

## 3. Mitigation without VMs, usable this week

### 3.1 Step 1, this week: move the heavy bots off the laptop (no code)

This is **the single change that solves "I can't use my laptop"**, and it exists
today:

1. **Create the iOS and Unity bots on the iMac.** Use "New bot" with the iMac as the
   machine, or have the lead call `create_bot` with `machine: "imac"`. In a linked
   project, workers can be pinned with `spawn_worker{machine: "imac"}`.
   - Their terminals, chat and browser stream to the laptop through the existing peer
     relays.
   - Their Device Hub, Xcode and Unity windows, if any, open on the **iMac's** screen,
     not the laptop's.
2. **Prepare the iMac** (owner actions):
   - Claude Code and/or Codex signed in. Node, if the bots' projects need it (the
     iMac reports none).
   - Unity Hub plus the same editor version (6000.6.0f1) with iOS Build Support,
     signed in with the same Unity ID. Personal has unlimited activations (§4.5).
   - Clone the repos there.
   - Keep it logged in. `xcodebuild test` needs an Aqua session (§3.6).
3. **Pin checks to the iMac.**
   - In each iOS or Unity repo's `.hermes/checks.toml`, set `machine = "imac"` and
     `heavy = true`. `machine = "mac"` would match any macOS computer, so the laptop
     too (`check_route.rs` `names`).
   - Because Unity isn't probed, add `needs = ["xcodebuild"]` so the check only lands
     on a Mac with Xcode.
4. **Free some laptop disk anyway.** This is the owner's call; the bots shouldn't do it.
   - `xcrun simctl delete unavailable`.
   - Remove the watchOS 26.5 runtime if unused.
   - Unity's 20 GB editor isn't needed on the laptop once Unity bots live on the iMac.

Example `checks.toml` for an iOS app and a Unity game. The commands are illustrative,
since each project has its own scheme and test assembly:

```toml
[[check]]
name = "ios-tests"
machine = "imac"
heavy = true
needs = ["xcodebuild"]
paths = ["App/**", "AppTests/**", "*.xcodeproj/**", "Package.resolved"]
# A check runs in a fresh checkout under the daemon's run/, so DerivedData stays inside it.
run = "UDID=$(xcrun simctl create ci-$$ com.apple.CoreSimulator.SimDeviceType.iPhone-17) && xcodebuild test -scheme App -destination \"platform=iOS Simulator,id=$UDID\" -derivedDataPath .dd -clonedSourcePackagesDirPath .spm -parallel-testing-enabled NO -jobs 4; s=$?; xcrun simctl delete $UDID; exit $s"

[[check]]
name = "unity-editmode"
machine = "imac"
heavy = true
needs = ["xcodebuild"]
paths = ["Assets/**", "Packages/**", "ProjectSettings/**"]
run = "\"/Applications/Unity/Hub/Editor/6000.6.0f1/Unity.app/Contents/MacOS/Unity\" -batchmode -nographics -projectPath . -runTests -testPlatform EditMode -testResults results.xml -logFile -"
```

### 3.2 Headless iOS: what each bot should do

All the flags below exist in Xcode 27's `xcodebuild -help` and `xcrun simctl help`
(local check):

- **Never launch the GUI.**
  - `xcodebuild build/test` and `xcrun simctl boot` run simulators headless; they have
    since Xcode 9. Bitrise and others run tests this way
    (https://discuss.bitrise.io/t/xcode-test-for-ios-v2-0-0/6809). `simctl boot` does
    not open the GUI (https://krypted.com/?p=50196).
  - A window appears only when something launches **Device Hub** (formerly
    Simulator.app): `open -a Simulator`, Xcode's Run button, or a tool that attaches a
    viewer.
  - If Device Hub is already running, it shows windows for booted devices, so one bot
    opening it surfaces the other's devices too. That is an inference from Xcode 27
    beta reports:
    https://claudeissues.com/issue/80425-ios-simulator-panel-attach-fails-with-no-booted-simulator-found-on-xcode-beta-de.
  - No environment variable disables GUI launching (none found).
- **One device per bot, by UDID.**
  - Create it with `xcrun simctl create "<bot> iPhone" <devicetype> [<runtime>]`. Build
    and test with `-destination 'platform=iOS Simulator,id=<UDID>'`. A name-based
    destination can make xcodebuild boot another device.
  - Bots already do this, and the guard already refuses `booted`/`all`.
  - `xcodebuild` has no device-set flag, so per-bot devices stay in the default set.
    `simctl --set` exists, but xcodebuild can't see those devices.
- **Per-bot build state:** `-derivedDataPath <bot dir>/xcode-derived`,
  `-clonedSourcePackagesDirPath <bot dir>/spm-checkouts`, `-packageCachePath <bot
  dir>/spm-cache`.
  - Concurrent builds sharing one SourcePackages folder fail with `manifest.db: database
    is locked`
    (https://forums.swift.org/t/swiftpm-how-to-prevent-resolve-packages-from-stymying-developer-productivity-local-packages/63363,
    SR-9651).
  - The clang module cache lives inside DerivedData
    (https://www.codementor.io/@vojtastavik/what-s-inside-the-derived-data-folder-r1bbp990t).
- **Bound the CPU.**
  - `-jobs 4` per bot, on a 10-core M4.
  - `-parallel-testing-enabled NO` and `-maximum-concurrent-test-simulator-destinations 1`.
    Parallel testing clones simulators ("Clone 1 of …"), which multiplies RAM use.
  - One booted simulator per bot. A stock simulator is about 4 GB of footprint, or about
    0.9 GB with background jobs disabled (`simctl boot --disabledJob=…`). Stock
    simulators thrash at about five on 16 GB (https://pkg.go.dev/github.com/mobai-app/simslim).
- **Look without a window:** `xcrun simctl io <UDID> screenshot <file>` and
  `recordVideo`. Shut the device down when done: `xcrun simctl shutdown <UDID>`.
- **Never run** `xed`, `open *.xcodeproj` or `open -a Xcode`. Plain `xcodebuild` never
  opens the Xcode GUI. If a bot must open something, `xed -b` keeps Xcode in the
  background.

### 3.3 Headless Unity

These flags come from the Unity 6 command-line reference
(https://docs.unity3d.com/6000.2/Documentation/Manual/EditorCommandLineArguments.html)
and the Test Framework reference
(https://docs.unity3d.com/Packages/com.unity.test-framework@1.4/manual/reference-command-line.html):

- **`-batchmode` opens no editor window.** Add `-nographics` for logic and EditMode
  tests: no graphics device is initialized.
- **`-nographics` breaks rendering paths in PlayMode tests:** cameras, RenderTextures,
  ReadPixels, light baking
  (https://discussions.unity.com/t/unity-5-3-seems-to-break-batchmode-nographics-on-headless-linux/610316).
  Run rendering PlayMode tests with `-batchmode` only, which still keeps the GPU and
  still opens no window. Alternatively, put them in a category and exclude it from the
  default run.
- **Tests:** `-runTests -testPlatform EditMode|PlayMode -testResults <xml> -testFilter …`.
  **Don't pass `-quit` with `-runTests`**; Unity says it isn't supported.
- **iOS export:** `-batchmode -buildTarget iOS -executeMethod Builder.Build`, which
  writes an Xcode project. That project is then built headless as in §3.2. The new CLI
  equivalent is `unity build --target iOS -o <path>` (local `unity build --help`).
- **The new `unity` CLI** (announced 2026-07-20, https://unity.com/blog/meet-the-unity-cli):
  `unity test --mode EditMode|PlayMode --report-format junit`, `unity status`, which
  lists open editors with project and PID, and `unity close <project>`. The Hub CLI
  (`Unity Hub -- --headless`) is deprecated since Hub 3.18.0
  (https://docs.unity.com/en-us/hub/hub-cli).
- **One editor per project folder.**
  - `Temp/UnityLockfile` refuses a second editor on the same project, interactive or
    batch
    (https://discussions.unity.com/t/error-it-looks-like-another-unity-instance-is-running-with-this-project-open/656599).
  - So two bots need two checkouts, each with its own `Library/`.
- **The Library folder is the real cost.** Here it is 3.3 GB for 55 MB of assets, and
  forum reports reach 42 GB, with first imports taking hours
  (https://discussions.unity.com/t/library-folder-huge/900715). Three ways to soften it:
  - **Keep one long-lived checkout per Unity bot** instead of today's "one git worktree
    per task" rule (`paths/prompt_sections.rs:262`), which forces a fresh import for every
    task. This changes the guidance for Unity projects only.
  - **Seed a new worktree's Library with an APFS clone:** `cp -cR <main>/Library
    <worktree>/Library`. `clonefile` is instant and copy-on-write. Inference, not
    tested here; Unity revalidates the copy on open.
  - **Unity Accelerator** on the iMac. It caches imports by asset, settings and
    platform, not by path, so worktrees share hits
    (https://docs.unity3d.com/Manual/UnityAccelerator.html). Pass
    `-EnableCacheServer -cacheServerEndpoint host:port`.

### 3.4 Stopping focus stealing

- **Not launching GUI apps** (§3.2, §3.3) removes almost all of it.
- **For the rest, launch in the background:** `open -g` (don't bring to the front) and
  `-j` (launch hidden) (`man open`). `open` has no `--background` flag; `-g` is it.
- **macOS has no system-wide "never steal focus" setting**
  (https://talk.macpowerusers.com/t/is-there-a-way-to-prevent-applications-from-stealing-focus-when-they-startup-on-macos/24539).
  An app that calls `activateIgnoringOtherApps` can still take focus.
- **Don't edit `LSUIElement`/`LSBackgroundOnly` in Xcode, Device Hub or Unity.** It
  breaks their code signatures and is overwritten on every update.
- **No `defaults` key stops Simulator or Device Hub activating.** The legacy
  `com.apple.iphonesimulator` domain holds only geometry and device preferences (local
  `defaults read`), and `com.apple.dt.Devices` is empty.
- **Spaces** (Dock → Options → Assign To Desktop N) keep windows off the owner's
  desktop, but an activation still switches Spaces. A weak fallback.
- **The strong guarantees** are another machine (§3.1), another macOS user (§3.6) or a
  VM (§4). Their windows physically can't appear on the owner's screen.

### 3.5 CPU and I/O priority

From `man taskpolicy`, `man setpriority` and Eclectic Light's QoS articles
(https://eclecticlight.co/2025/05/09/what-is-quality-of-service-and-how-does-it-matter/,
https://eclecticlight.co/2024/12/05/inside-m4-chips-cpu-core-management/):

- **`taskpolicy -c utility <cmd>`** clamps the whole process tree to Utility QoS. "All
  children of the specified program also inherit these policies." It still uses
  P cores, but yields to the owner's interactive work. **Recommended for every bot
  session.**
- **`taskpolicy -b`** (PRIO_DARWIN_BG) confines the tree to the **E cores** at about
  1 GHz, and throttles disk and network I/O.
  - On this M4 that means 6 slow cores. Expect builds 3-5× slower (inference; no
    measured xcodebuild figure found).
  - Network throttling could also slow the bot's own API streaming. Use it only for a
    bot the owner explicitly wants "out of the way".
- **`taskpolicy -d throttle`** throttles disk I/O only. It is useful for a Unity
  reimport.
- **`nice`** only shifts priority within a band. It doesn't move work to E cores.
- **There is no per-process GPU cap on macOS.** The levers are `-nographics`, fewer
  booted simulators, and pausing (SIGSTOP/SIGCONT).
- **Measured here:** `taskpolicy` and `nice` exec in place (§1). Wrapping `claude` in
  `/usr/sbin/taskpolicy -c utility` keeps the session-root PID that bus identity
  (`bus_auth/session.rs`) records.

### 3.6 Running bots under a separate macOS user

Running the bots under their own macOS user isolates them fully, but it's heavy:

- **What it gives.**
  - Each user has its own WindowServer session. With Fast User Switching, a background
    session keeps running but gets no display, keyboard or mouse, so **its windows can
    never reach the owner's screen**
    (https://eclecticlight.co/2023/03/22/fast-user-switching-how-it-works-and-when-to-use-it).
  - CoreSimulator, DerivedData, caches, keychains and `~/.claude` all become per user.
- **What it costs.**
  - `xcodebuild test` needs an **Aqua (GUI) session**. Over SSH or from a LaunchDaemon
    it fails with a `testmanagerd` error. The bot user has to be logged in, through
    Fast User Switching or auto-login (https://developer.apple.com/forums/thread/727333,
    https://developer.apple.com/forums/thread/696859).
  - The user is created with `sysadminctl -addUser`. Processes run in its session with
    `launchctl asuser <uid> …`.
  - Simulators and Unity in a *backgrounded* FUS session are **unverified**.
  - The user needs its own Unity sign-in, its own Claude/Codex sign-in, and signing
    certificates in its own keychain (`security unlock-keychain`,
    `set-key-partition-list`).
  - A second session costs RAM on a 24 GB machine.
- **For Gravity it's a bigger change than it looks.**
  - The daemon, its `~/.thehermes` and the bus sockets belong to the owner's user.
  - A bot under another user is effectively **another computer**: a second hermesd
    under that user, linked as a peer. That's the same shape as the VM design (§5),
    without the hardware boundary.
  - So it's a stopgap only if the owner has no second Mac. With the iMac available,
    step 1 is simpler and stronger.

### 3.7 Step 2: small code changes Gravity could ship (about 3-5 days)

Per-bot and per-project settings, and the files each one touches:

| Change | What | Files |
|---|---|---|
| **Per-bot Apple and Unity build folders** | Like `CARGO_TARGET_DIR`: export `GRAVITY_XCODE_DERIVED=<bot dir>/xcode-derived`, `GRAVITY_SPM_DIR=<bot dir>/spm`, `GRAVITY_SIM_NAME=<bot>-<id8>`, plus `TMPDIR=<bot dir>/tmp`. `xcodebuild` reads no env var for `-derivedDataPath`, so the prompt and guard (below) make bots pass them. Remove the folders on bot delete, like `remove_bot_target` | `workers/target.rs` (or a sibling `workers/apple_dirs.rs`), `workers/scratch.rs:37` `bot_env`, `cleanup/remove.rs` |
| **Priority wrapper** | `[bots] priority = "utility" \| "background" \| "normal"` (default `utility` on macOS). The PTY runs `/usr/sbin/taskpolicy -c utility <claude> …`. The PID is unchanged (measured). Codex's App Server gets the same treatment. Windows: `BELOW_NORMAL_PRIORITY_CLASS` on the Job Object | `runtime/pty.rs:52`, `runtime/codex/transport.rs`, `config.rs`, `ops/hermesd.example.toml` |
| **Guard rules against GUI launches** | Deny with a reason: `open -a Simulator\|DeviceHub\|Xcode\|Unity*` without `-g`; `open *.xcodeproj\|*.xcworkspace`; `xed` without `-b`; `xcodebuild` without `-derivedDataPath` (when `GRAVITY_XCODE_DERIVED` is set); Unity's binary without `-batchmode`; `unity open`. Treat `tart run`, like `colima start`, as a VM start | `bot_permissions/guard/commands.rs` (it's at 382 of 400 lines, so a new `guard/apple.rs` would be needed), `bot_permissions/tests/guard_cases.rs` |
| **Prompt guidance** | A "Building Apple and Unity projects on a shared Mac" section, shown when the machine has `xcodebuild`: headless only, your UDID, your folders, `-jobs 4`, no parallel testing, `simctl io` screenshots, Unity `-batchmode` (`-nographics` unless rendering), one long-lived Unity checkout, shut down your simulator when idle | `paths/prompt_sections.rs` (next to `shared_computer`), `paths/prompt.rs`, `paths/prompt_tests.rs` |
| **Unity probe** | Report `unity` (from `~/.unity/bin/unity --version`, or the Hub editors folder) so checks and placement can say `needs = ["unity"]` | `machine_tools.rs:20` (`TOOLS` is name + flag on `PATH`; Unity needs a special case), `tool_path.rs` |
| **A heavy slot for bots' own builds** | `hermesd heavy -- <cmd>`: waits for a slot in the machine's `check_heavy` ledger, then runs the command. The prompt asks bots to wrap `xcodebuild`, Unity imports and `cargo build` in it, so two bots' builds queue instead of thrashing. MCP alternative: `acquire_heavy`/`release_heavy` | `main.rs` (subcommand), `prs/check_jobs.rs`/`db` (`check_heavy`), `bot_permissions/settings.rs` (allow the exact `hermesd heavy` path, as for other hermesd commands) |
| Per-project overrides | None needed in the MVP. A project could later set `[apple] jobs = 4` in a `.hermes/build.toml`. Not worth it yet | n/a |

---

## 4. A macOS VM per bot on Apple silicon

### 4.1 The two-VM limit and the licence

- **Enforced by the framework.** Starting a third concurrent **macOS** guest fails with
  `VZErrorDomain` code 6, `VZErrorVirtualMachineLimitExceeded`: "The maximum supported
  number of active virtual machines has been reached."
  - It counts running macOS guests on the host, whichever app started them (tart, UTM,
    lima, Parallels' VZ mode). Inference: the counting is in the framework and kernel,
    not per app.
  - Linux guests don't count.
  - Kernel patches can lift the limit, and they violate the licence.
  - Sources:
    https://eclecticlight.co/2022/08/04/virtualisation-on-apple-silicon-macs-8-how-apple-limits-vms/,
    https://khronokernel.com/macos/2023/08/08/AS-VM.html,
    https://juergenkoller.software/en/blog/more-than-two-macos-vms-on-apple-silicon.html.
  - One unconfirmed report on macOS 26.5 says a slot wasn't released after a guest shut
    down; a host reboot fixed it (https://developer.apple.com/forums/thread/830119).
- **The licence.** The macOS SLA §2B(iii) has carried the same clause since Lion. It
  allows installing, using and running "up to two (2) additional copies or instances"
  of macOS in virtual operating system environments, on each Apple-branded computer you
  own or control that is already running macOS.
  - Allowed purposes: software development, testing during development, macOS Server,
    or personal non-commercial use.
  - It excludes service bureau, time-sharing and terminal-sharing.
  - Sources: Apple's Lion-era PDF https://www.apple.com/legal/sla/docs/macosx1072.pdf;
    the Big Sur wording quoted at
    https://threedots.ovh/blog/2020/12/macos-eula-licensing-restrictions-affecting-virtualisation/.
  - I couldn't extract the macOS 26 or 27 SLA text from https://www.apple.com/legal/sla/.
    Recheck it before shipping.
  - Developers testing in parallel keep hitting the cap
    (https://developer.apple.com/forums/thread/826102).
- **For Gravity:**
  - Bots doing development and testing on the owner's own Macs fit the allowed
    purposes.
  - **Two macOS computers per Mac, ever**, counted across everything running on that
    Mac.
  - Never offer macOS computers to anyone but the owner (no "time-sharing").

### 4.2 Choosing the tool: tart, lima, UTM, or Virtualization.framework directly

| | **tart** | lima (vz, `template:macos`) | UTM | Virtualization.framework from Rust |
|---|---|---|---|---|
| Status | **2.41.0, 2026-10-13**, `github.com/openai/tart`. Cirrus Labs' team joined OpenAI's agent-infrastructure team; reported as no longer charging licence fees (secondary source: https://aiidelist.com/blog/openai-brings-tart-team-into-agent-infrastructure) | 2.2.0 installed here. macOS guests since 2.1, experimental | 5.0.6 beta adds macOS 27 features (USB passthrough, VZ snapshots) (https://newreleases.io/project/github/utmapp/UTM/release/v5.0.6) | `objc2-virtualization` or a Swift helper |
| Licence | **FSL-1.1-ALv2** (repo `LICENSE`, "Copyright 2022-2026 OpenAI"): internal use is fine, each release becomes Apache-2.0 after 2 years | Apache-2.0 | Apache-2.0 | n/a |
| Images | OCI registries. `ghcr.io/cirruslabs/macos-{tahoe,golden-gate,…}-{vanilla,base,xcode}` | Apple IPSW only (the template pins 26.5.2) | IPSW, manual | IPSW |
| Headless | `--no-graphics`: no window, no Dock icon (https://flaviocopes.com/mac-test-vm/) | `video.display: none\|vnc` | GUI app; `utmctl` | Yes |
| VNC | **`--vnc-experimental`** (framework VNC, from boot, during install) or `--vnc` (guest Screen Sharing) | `display: vnc` | Built-in viewer | `_VZVNCServer` is private API |
| Commands into the guest | **`tart exec`** over vsock through the Tart Guest Agent, which every non-vanilla Cirrus image ships (https://crowci.dev/dev/configuration/env-vars/backend-tart/). Also SSH | SSH, provisioning scripts | AppleScript | Our own agent |
| First-boot setup | `--provisioning-opts` (user, password, auto-login, SSH) on a **macOS 27 host and guest** (`Run.swift`, Apple's `VZMacGuestProvisioningOptions`) | lima's own provisioning | Manual | Same API |
| Fit for hermesd | **Best**: one CLI, JSON output, used by Cirrus CI, Crabbox, Nomad drivers | Good for Linux; macOS guest experimental | Not daemon-friendly | Most control, most work: install, NVRAM, disks, networking, VNC |

**Recommendation: tart, through its CLI**, behind the report's `ComputerProvider`
trait. Keep a direct-framework provider as a later option only if tart's licence or
direction changes. There's no tart library API.
- Orchard, Cirrus' orchestrator, has a REST API but targets fleets. One daemon managing
  two VMs on its own host doesn't need it.
- **bollard doesn't apply to macOS guests.** No Docker is involved. Keep it for the
  Ubuntu and Windows providers.

### 4.3 What a tart macOS guest needs (disk, RAM, cores, boot, snapshots)

| | Value | Source |
|---|---|---|
| Base image (`macos-tahoe-base`) | ~25 GB download (about half an hour), **33 GB on disk** after clone | https://flaviocopes.com/mac-test-vm/, tart README |
| Xcode image template | `disk_size` **140 GB**, 4 CPU, 8 GB RAM. Installs Xcode from `.xip`, `xcodebuild -downloadPlatform iOS` / `-downloadAllPlatforms`, Flutter, fastlane, CocoaPods, Android SDK, and Homebrew **Codex and Claude Code**. SSH `admin`/`admin` | https://raw.githubusercontent.com/cirruslabs/macos-image-templates/main/templates/xcode.pkr.hcl |
| Our golden image | macOS 27 + Xcode 27 + the iOS 27 runtime + Unity 6000.6 with iOS (20 GB measured here) + Node, Claude Code, Codex, hermesd. **About 70-90 GB used** (inference from the figures above) | n/a |
| Per-bot clone | `tart clone golden gravity-<bot>`. On APFS a clone is copy-on-write and shares the golden's blocks until written (tart FAQ: `~/.tart/vms`, `~/.tart/cache/OCIs`; `tart clone` prunes caches up to `--prune-limit`, default 100 GB). Each bot's builds add **DerivedData, a Library per project, simulator data**: plan **20-50 GB** of growth per guest | https://tart.run/faq/ |
| RAM and cores per guest | 8 GB / 4 vCPU (Cirrus default). With a booted simulator plus Xcode, 8 GB is tight; Unity editor plus a simulator wants 12 GB | template; §3.2 simulator figures |
| Boot | About 15 s from cold for a configured guest (one report); the first install from IPSW takes much longer | https://flaviocopes.com/mac-test-vm/ |
| Suspend | `tart run --suspendable` (no audio or entropy devices), then `tart suspend` | `Run.swift` |
| Snapshots | No `tart snapshot`. Use **a clone as a checkpoint** (`tart clone gravity-x gravity-x@2026-10-14`, copy-on-write). macOS 27 adds **DiskImageKit** layered ASIF images (read-only base, cache and overlay layers), which UTM already uses for VZ snapshots. Tart support is unknown | https://eclecticlight.co/2026/06/12/macos-virtualisation-is-leaping-forward-in-golden-gate/, https://bitrise.io/blog/post/wwdc26-the-virtualization-framework-updates-that-matter-for-large-mac-fleets |
| Host requirement | **macOS 15+: Virtualization.framework needs an unlocked login keychain to run a VM.** hermesd's LaunchAgent in the owner's logged-in session is fine. A dedicated Mac mini needs auto-login | https://tart.run/faq/ |

**Totals for two guests on one Mac:**
- **Disk:** golden 80-90 GB, plus 2 × 20-50 GB of growth, plus tart's OCI cache if the
  image was pulled (it is cloned from too). That's about **150-200 GB free, before the
  host's own needs**.
- **RAM:** 2 × 8-12 GB for the guests plus 8 GB for the host. That's **about 32 GB
  minimum, 48 GB comfortable** with Unity.

The Air has 16 GiB free and 24 GB of RAM: it fits neither.

### 4.4 Metal, Unity and the Simulator inside a guest

- **The paravirtualized GPU** (`VZMacGraphicsDeviceConfiguration`, shown in the guest as
  "Apple Paravirtual device") gives guests Metal. It is Metal only, so OpenGL apps
  don't run (https://kb.parallels.com/124138).
  - GPU code runs directly on the host hardware: a Geekbench Metal score of 102,282 in
    the VM against 110,960 on the host.
  - But some Apple Metal samples that run on the host **failed in the VM**, and some
    apps refuse the virtual GPU
    (https://eclecticlight.co/2023/10/26/how-good-is-gpu-access-for-apple-silicon-virtual-machines/).
  - GitHub's arm64 macOS runners, which are VZ guests, list "nested virtualization and
    **Metal Performance Shaders (MPS)** are not supported" in some doc versions
    (https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
  - Nothing documents which Metal GPU family a guest reports. The QEMU `apple-gfx`
    patches describe a "fixed baseline" of 3D and compute support, independent of the
    host GPU (https://patchew.org/QEMU/20241223221645.29911-1-phil@philjordan.eu/).
- **Unity in a guest: no public evidence either way.**
  - Unity's system requirements describe workstations "running without emulation,
    container or compatibility layer", so a VM is formally outside support
    (https://docs.unity3d.com/2022.2/Documentation/Manual/system-requirements.html).
  - Inference:
    - **batchmode with `-nographics`** (EditMode tests, iOS export) needs no GPU, so it
      should work;
    - **the editor UI and Play Mode** depend on how much of Metal the paravirtual
      device gives Unity. That's plausible, but unproven.
  - **A one-hour test is needed before committing**: open the editor, enter Play Mode,
    run PlayMode rendering tests, build for iOS. It belongs in phase M0 (§7).
- **Xcode and the iOS Simulator in a guest work, with evidence.**
  - Cirrus' Xcode images download iOS runtimes, and `expected_runtimes_file` checks them
    with `simctl list runtimes`.
  - GitHub's arm64 macOS runners and CircleCI's macOS executors run `xcodebuild test`
    on simulators daily in virtualized macOS. The VZ part is GitHub's statement; for
    CircleCI it's my inference.
  - The known cost is **first-boot indexing**: a fresh simulator can hold 3 vCPUs for
    1-2 minutes. The fix is a warm simulator kept in the golden image
    (https://support.circleci.com/hc/en-us/articles/46737392770203-Reducing-iOS-Simulator-CPU-Overhead-on-macOS-Executors).
- **Physical iPhone.**
  - **USB:** macOS 27 adds USB passthrough into VZ guests through the new Accessory
    Access framework. It needs a menu-bar approval per device, the
    `com.apple.developer.accessory-access.usb` entitlement on the host app, and
    reportedly macOS 27 on host and guest.
    - Storage, FIDO keys and SD cards work. Isochronous devices such as webcams don't.
    - **iPhones aren't mentioned anywhere.** One developer saw the connect callback not
      fire in the beta. UTM 5.0.6 beta exposes it; tart support is unknown. Sources:
      https://eclecticlight.co/2026/06/12/macos-virtualisation-is-leaping-forward-in-golden-gate/,
      https://next.ink/246929/macos-27-donne-un-coup-de-fouet-a-sa-virtualisation,
      https://developer.apple.com/forums/thread/830167.
  - **Network:** Xcode 27's Device Hub "adds a new way to pair iPhone, iPad and Apple
    Watch running iOS 27 … over a network" (Xcode 27 release notes). A guest on
    bridged networking (`tart run --net-bridged en0`) might pair the owner's iPhone
    without USB. Unverified.
  - **Plan:** device installs (the `iphone` release target) stay on a real Mac.
    Simulators run in guests.

### 4.5 Identity: Apple ID, signing, Unity seats

- **Apple ID in a guest.** Since macOS 15, with **both host and guest on 15 or later**,
  a guest can sign in to an Apple Account and iCloud.
  - Each VM gets a distinct identity derived from the host's Secure Enclave.
  - Moving a VM to another Mac creates a new identity, so the user signs in again.
  - The framework detects two copies of one VM running at once.
  - **Mac App Store sign-in is still unsupported in a guest**, so Xcode comes from a
    `.xip`, not the App Store.
  - Sources:
    https://eclecticlight.co/2024/07/12/sequoia-virtualisation-and-apple-id/,
    https://www.computerworld.com/article/2164707/for-it-and-devops-sequoia-brings-icloud-to-vm-macs.html,
    https://docs.parallels.com/landing/pdfm-ug/parallels-desktop-for-mac-26-users-guide/advanced-topics/using-other-operating-systems-on-your-mac/running-macos-virtual-machines/signing-into-apple-account-on-macos-virtual-machines.
  - Inference for tart: a clone of the golden image is a new VM, so each bot's guest
    needs its own Apple sign-in if it signs in at all. Keep the golden image signed
    **out**.
- **Signing without an Apple ID in the guest (recommended).** Simulator builds need no
  signing identity. For device or archive builds, pass `xcodebuild -allowProvisioningUpdates
  -authenticationKeyPath <p8> -authenticationKeyID <id> -authenticationKeyIssuerID <iss>`.
  - These flags are in Xcode 27's `xcodebuild -help`, which says the flag requires
    "a developer account … in Xcode's Accounts settings **or** an App Store Connect
    authentication key".
  - The host vault keeps the `.p8` key (`secrets/computer-<id>/`), and the provider
    injects it through `tart exec`.
  - Alternative: export a distribution certificate and profiles into the guest's
    keychain.
- **Unity seats.**
  - **Personal has unlimited activations.** Pro, Industry and Enterprise allow **2
    simultaneous machine activations per seat**; a third is refused
    (https://support.unity.com/hc/en-us/articles/4402049280276).
  - Personal has no serial, so activate by signing in with Hub or the `unity` CLI.
    `-serial` batchmode activation is for paid plans
    (https://docs.unity3d.com/Manual/ManagingYourUnityLicense.html).
  - Inference: a VM has its own machine identity, so **each guest counts as a machine**.
    On Pro, laptop + iMac + 2 guests would need 2 seats. On Personal it's free, subject
    to Personal's revenue rules.
  - Each guest needs a Unity sign-in once. A golden image signed in would be cloned
    with one machine identity, so sign in after cloning.

### 4.6 The `tart` provider in hermesd

This reworks the report's §7.1: **`tart` comes first** on Macs, and **bollard
providers** (Ubuntu, Windows) come after it.

```
ComputerProvider (trait, computer/provider.rs)
├── TartMacos     computer/tart.rs     — CLI via tokio::process, macOS hosts only   ← first
├── DockerUbuntu  computer/ubuntu.rs   — bollard (report P1)                         ← second
├── DockurWindows computer/windows.rs  — bollard (report P3)                         ← third
└── Fake          computer/fake.rs     — tests
```

**Lifecycle through tart's CLI** (flags from `Sources/tart/Commands/Run.swift` and the
tart docs):

| Step | Command | Notes |
|---|---|---|
| Probe | `tart --version`; `tart list --format json` | Hide the provider when tart is absent. Count running macOS guests, including ones Gravity didn't start, against the cap of 2 |
| Golden image (owner action, once) | `tart clone ghcr.io/cirruslabs/macos-tahoe-xcode:<v> gravity-golden`, or build ours with the tart Packer plugin from `macos-*-base`. Then install Unity and sign out | Ours: Xcode 27, the iOS 27 runtime, a warm simulator, Unity 6000.6 + iOS, Node, Claude Code, Codex, hermesd (the macOS arm64 build already ships) |
| Create | `tart clone gravity-golden gravity-<botdir>`; `tart set gravity-<botdir> --cpu 4 --memory 8192 [--disk-size N]` | A copy-on-write clone; seconds |
| First-boot setup | `--provisioning-opts username=…,password=…,logsInAutomatically=true,enablesRemoteLogin=false` | **macOS 27 host and guest.** Otherwise the image's `admin`/`admin`, changed at once |
| Run | `tart run gravity-<botdir> --no-graphics --vnc-experimental --net-softnet --net-softnet-expose <hostport>:49777 [--dir artifacts:<path>:ro] [--suspendable]` as a **long-lived child of hermesd**, recorded in the holders ledger (`holders.rs`) | **No host window, no Dock icon.** tart prints `VNC server is running at vnc://:<password>@127.0.0.1:<port>`; the provider parses it. Softnet isolates the guest from the host ("not possible to access the host", tart FAQ); `--net-softnet-expose` lets the host dial the guest's hermesd on loopback |
| Provision | `tart exec gravity-<botdir> -- …`: write `hermesd.toml`, the peer token, the Claude/Codex credentials, the ASC key; start the guest hermesd as a LaunchAgent in the guest's auto-login session | Over vsock, needing no guest networking. Requires the Tart Guest Agent (in every non-vanilla Cirrus image) |
| Pair | The host dials `127.0.0.1:<hostport>` with the minted peer token. The guest accepts only that token (the report's §7.2 item 3) | Unchanged from the report |
| Stop / suspend | `tart stop gravity-<botdir> [--timeout 120]`; `tart suspend` | Suspend only with `--suspendable` |
| Reset | `tart delete gravity-<botdir>`, then create again from the golden | The bot's identity and history stay on the host |
| Checkpoint | `tart clone gravity-<botdir> gravity-<botdir>@<date>` | Until DiskImageKit snapshots arrive in tart |
| Reconcile | `tart list --format json` each tick; name prefix `gravity-` plus a computer row `(bot_id, provider='tart', vm_name, state, vnc_port, peer_port)` | Orphans after retention, as in the report |
| Failures | A VZ code 6 from `tart run` means "two macOS computers are already running on this Mac": say so on the bot card, and don't retry in a loop. A locked keychain at start means "log in to this Mac" | Report §7.7 plus these two |

**Networking caveat.**
- On tart's default NAT, a guest reaches host services bound to `0.0.0.0` (tart FAQ).
  hermesd binds `127.0.0.1` and the tailnet address. Inference: the tailnet address is
  probably reachable from a NAT guest through the host.
- **Use Softnet** and expose only the guest's hermesd port. That gives the asymmetric
  "host dials guest" model from the report with no host listener the guest can reach.
- If a bot needs a host dev server, add `--net-softnet-allow` for that one address.

### 4.7 The Computer tab for a macOS guest

- **`--vnc-experimental`** is Virtualization.framework's own VNC server.
  - It works from boot, in recovery and during installation, needs no setup in the
    guest, and is password-protected on host loopback.
  - Tart's help warns it is "experimental and there may be bugs".
  - Feed it into the report's §8.3 (d) VNC-to-JPEG-frames bridge (`computer/vnc.rs`
    with `vnc-rs`) and its `watch_computer`/`computer_frame`/`computer_input` frames.
    The phone and linked computers get it for free.
- **`--vnc`** uses the guest's Screen Sharing instead. It has better encodings, clipboard
  and drag-and-drop, but needs Screen Sharing and Remote Login enabled in the guest,
  and starts only once macOS is up. Keep it as a fallback, or as the phase-3 RFB tunnel
  target for smoother Take control.
- **The owner's screen stays free.**
  - VNC alone opens no window ("VNC runs without a window unless `--graphics`").
  - The guest's Device Hub, Xcode and Unity windows exist only inside the guest's
    framebuffer.
  - The owner sees them only when opening the bot's Computer tab.
- **Take control** maps `browser_input`-shaped events to RFB pointer and key events,
  as in the report.
  - macOS guests want the trackpad and keyboard devices (the defaults); don't pass
    `--no-trackpad`.
  - Cmd-key combos travel as RFB keysyms (Super/Meta). This needs testing.

---

## 5. Where a second Mac beats VMs on the laptop

| Option | Owner's laptop free? | Isolation between bots | Setup | Cost |
|---|---|---|---|---|
| **A. Bots on the iMac (linked computer)** | **Yes, fully** | Same user on the iMac, like today's laptop: per-bot dirs and guard rules (§3) | **Today**: install tools, create bots there | 0 |
| B. Two tart guests on the iMac | Yes | **Strong** (VM per bot) | Phases M0-M2 (§7) | 0 if the iMac has ≥ 32 GB RAM and ~200 GB free (it has 274 GB) |
| C. Mac mini as a dedicated bot host (A or B there) | Yes, and the iMac stays usable too | A or B | A day of setup | See below |
| D. Two tart guests on the laptop | Partly (CPU, RAM and heat are still shared) | Strong | M0-M2 | Impossible at 16 GiB free / 24 GB RAM. Needs ≥ 200 GB freed and still leaves ~8 GB for the owner |
| E. Separate macOS user on the laptop | Windows, yes; CPU and RAM, no | Medium | Days (a second hermesd as a peer) | 0 |

**When a second Mac wins: always, for this owner.**
- The laptop is a fanless Air with 4 P cores. Anything heavy running on it, VM or not,
  costs the owner battery, heat and responsiveness.
- VMs on the laptop only fix the windows, and they cost disk the laptop doesn't have.

**What to buy, if anything.** Apple's spec page, 2026-10-14
(https://www.apple.com/mac-mini/specs/); prices per the research agent from
https://daringfireball.net/2026/08/configurations_and_pricing_for_new_mac_minis_and_mac_studios.
Verify before buying.

| Need | Configuration | Approx. price (US) |
|---|---|---|
| A + the iMac stays the owner's | Nothing. The iMac hosts bots as linked-computer bots | 0 |
| Two macOS guests, iOS only (Xcode + 1 simulator each) | Mac mini **M6, 32 GB, 1 TB** (12-core CPU, 12-core GPU; 32 GB is its maximum) | about $900 + $400 + $500 ≈ **$1,800** |
| Two macOS guests with **Unity** + Xcode + simulators, comfortable | Mac mini **M5 Pro, 48 GB, 1 TB** (15-core CPU, 16-core GPU, Thunderbolt 5) | about $1,700 + $600 + $300 ≈ **$2,600** |
| Heaviest (two Unity editors in Play Mode plus simulators, or 2 guests plus host-side bots) | M5 Pro, **64 GB**, 2 TB | about $3,500 |

**Thresholds** for "2 macOS guests with Xcode and Unity on one host":
- **RAM:** 32 GB is the floor, 48 GB is recommended, 64 GB if host-side bots run too.
- **Free disk:** 200 GB after the OS; 1 TB internal recommended.
- **CPU:** 10 or more cores, so each guest gets 4 vCPU.
- **Thermals:** a machine with a fan.
- **Network:** Ethernet, to keep the tailnet link to the laptop steady.

An **iMac with 24 GB** could run one guest plus host-side bots. With **32 GB** it could
run two guests for iOS work (Q1).

---

## 6. Security notes specific to macOS guests

These add to the report's §7.6:

- **A guest is a strong boundary for files and processes.** The guest bot can't read
  the host's `~/.thehermes/secrets`, Keychain, `~/.claude` or other bots' workspaces.
- **Softnet is required, not optional.** Default NAT lets guests reach host services
  bound to `0.0.0.0`, and probably the tailnet address.
- **The guest holds a real Claude/Codex credential, possibly an App Store Connect key,
  and a Unity sign-in.** Give each guest its own where revocable: one ASC key per
  computer with the Developer role only.
- **tart's VNC password is printed on tart's stdout.** hermesd reads it from the child's
  pipe and never logs it. The VNC port is on loopback; host-side bots can reach
  loopback, so the password is what protects it.
- **Host-side bots can run `tart`** and control every guest, as with the Docker socket
  in the report. The guard should deny `tart` to bots (except DevOps, if wanted).
- **Default credentials** `admin`/`admin` in Cirrus images must be replaced at
  provisioning (`--provisioning-opts` or `tart exec … sysadminctl -resetPasswordFor`).

---

## 7. Revised MVP and phases

This reuses the report's peer-based design throughout: a guest hermesd in computer
mode, paired as a `computer` peer with asymmetric trust, with the Computer tab on the
VNC-to-frames bridge. Effort is for one engineer or bot team, roughly.

| Phase | What | Effort | Touched modules |
|---|---|---|---|
| **S1. This week, no code** (§3.1) | iOS/Unity bots on the iMac; headless instructions in each bot's `instructions`; `checks.toml` with `machine = "imac"`, `heavy = true`; free laptop disk | Owner: half a day | Project repos' `.hermes/checks.toml`; bot instructions |
| **S2. Shared-Mac hygiene** (§3.7) | Per-bot Apple/Unity folders; `taskpolicy -c utility` sessions; guard rules against GUI launches and `tart`; prompt section; Unity probe; `hermesd heavy` | **3-5 days** | `workers/target.rs` (+ `workers/apple_dirs.rs`), `workers/scratch.rs`, `runtime/pty.rs`, `runtime/codex/transport.rs`, `bot_permissions/guard/` (+ `guard/apple.rs`), `bot_permissions/settings.rs`, `paths/prompt_sections.rs`, `machine_tools.rs`, `main.rs`, `prs/check_jobs.rs`, `config.rs`, `ops/hermesd.example.toml`, `cleanup/remove.rs`, tests |
| **M0. Feasibility test (owner plus a bot, 1 day)** | On the iMac: install tart, clone a Cirrus Xcode image, run `--no-graphics --vnc-experimental`, open Unity editor + Play Mode + a PlayMode rendering test + an iOS export, run `xcodebuild test` on a simulator, measure RAM, disk growth and boot time. Decide whether Unity bots go in guests or stay on the host | 1 day | none (a written result) |
| **M1. The macOS computer per bot (max 2 per Mac)** | `TartMacos` provider (§4.6); the golden image recipe (Packer + tart plugin); the guest-mode hermesd (macOS arm64 already ships, so no new release target); the `computer` peer class with asymmetric trust and the narrow host surface (report §7.2); pairing over Softnet-exposed loopback; the Computer tab with watch + Take control on desktop, watch on phone (report §8, MVP scope) | **4-5 weeks** | New `computer.rs` + `computer/{provider,tart,pairing,reconcile,vnc,input}.rs`; `peer/owner_trust.rs`, new `peer/computer.rs`, `peer/socket.rs`, `peer/roster.rs`, `peer/receive.rs`; `main.rs` (computer mode); `config.rs` (`[computers] enabled, tart_bin, golden, max_running = 2, cpus, memory_gb`); schema migration (`computer` table, `peer.kind`); `ws/computer.rs`, `ws/dispatch.rs`, `ws/owner_auth.rs`, `app.rs` capabilities; `holders.rs` (tart child); `bot_permissions/guard` (deny `tart`); `Cargo.toml` (`vnc-rs`, a JPEG encoder; **no bollard yet**); `apps/desktop/src/components/bot/BotTabs.tsx`, new `components/computer/*`, `protocol/*`, `test/fakeDaemon.ts`; phone `ComputerPane.swift`; `tests/computers.rs` with a fake provider and two in-process daemons; `docs/computers.md`, `docs/peer-bots.md`, `docs/architecture.md`; `ops/computer/macos/` (Packer template, provisioning script) |
| M1 differences from the report's MVP | Drops: bollard, Docker endpoint resolution, the Ubuntu image, new Linux release targets. Adds: the tart CLI wrapper, parsing tart's stdout, Softnet, provisioning through `tart exec`, the 2-VM cap and VZ code 6, the unlocked-keychain precondition, the ASC key and Unity sign-in steps in the golden runbook | (net similar to the report's 3-4 weeks, plus image work) | n/a |
| **M2. Hardening** | Checkpoints (clone) and reset; suspend on idle (`--suspendable`); disk-growth alerts per guest; warm-simulator golden; Unity Accelerator in the golden; `--vnc` / RFB tunnel for smoother control; signed owner approvals for control across linked computers (report phase 5) | 2-3 weeks | `computer/tart.rs`, `computer/reconcile.rs`, `ws/computer.rs`, desktop app |
| **U. Ubuntu computers for web and backend bots** | The report's MVP (P1 containers via bollard) as the second provider | 3-4 weeks (much shared with M1: peer class, tab and bridge already exist, so likely 2 weeks) | Report §9 table, minus what M1 built |
| **W. Windows computers** | dockur on the Windows PC (report phase 2) | 2-3 weeks | Report §9 phase 2 |
| Later | macOS 27 USB passthrough of an iPhone (if Apple and tart support it); DiskImageKit snapshots; Device Hub network pairing from a guest | n/a | n/a |

**M1 acceptance.**
1. On the iMac, the owner creates "iOS Dev" with a macOS computer. The bot appears on
   the laptop as a stand-in.
2. Its Computer tab shows the guest's desktop: the guest's Device Hub with its
   simulator, and Xcode if opened.
3. A second bot, "Game Dev", gets the second guest and runs Unity there.
4. **No window opens on the iMac's own screen, and none on the laptop's.**
5. Take control signs in somewhere in the guest, and Give back control hands it back.
6. A third macOS computer is refused with a clear reason.
7. Reset gives a clean guest from the golden image, with the same bot.
8. A shell in the guest pretending to be its hermesd can't reach other bots or the
   host's projects, and can't reach the host over the network (Softnet).

---

## 8. Open questions for the owner

1. **Each Mac's chip, RAM and free disk.**
   - Laptop: measured, M4 / 24 GB / 16 GiB free.
   - iMac: Apple silicon, 274 GB free, but **which chip and how much RAM?**
     `sysctl -n machdep.cpu.brand_string hw.memsize` on the iMac answers it.
   - Any other Mac?
   - Is the iMac in use by someone else, or free to be a bot host?
2. **Move the iOS and Unity bots to the iMac now** (step S1)? Should the laptop then run
   only light bots, or none during study hours?
3. **Freeing laptop disk:** may unused simulator runtimes, `simctl delete unavailable`
   and the laptop's Unity editor (20 GB) go, once Unity bots live on the iMac?
4. **Unity plan:** Personal (assumed, measured here) or Pro? If Pro, how many seats? Each
   VM likely counts as a machine.
5. **Apple Developer account:** may Gravity use an **App Store Connect API key** for
   signing in guests and on the iMac, instead of an Apple ID signed in to each machine?
6. **Hardware:** buy a Mac mini as the bot host? M6 32 GB / 1 TB (about $1,800) for
   iOS-only guests, or M5 Pro 48 GB / 1 TB (about $2,600) for Unity plus iOS.
7. **Priority:** run every bot session at `taskpolicy -c utility` by default (proposed)?
   Should any bot be `-b` (E cores only, much slower)?
8. **Guard strictness:** deny GUI launches outright (proposed), or only warn? May a bot
   ever open Xcode or Unity's editor UI on a host machine, for example on the iMac
   when nobody uses it?
9. **Unity checkouts:** for Unity projects, replace "one worktree per task" with "one
   long-lived checkout per bot" to avoid reimports (proposed)?
10. **Order after macOS:** Ubuntu computers next, then Windows (assumed)? Or Windows
    first, for the 0.19 "a Windows bot compiles and installs it" rule?
11. **Phone iPhone installs:** keep them on a real Mac (assumed), or try macOS 27 USB or
    network pairing into a guest later?
12. **The report's questions Q1-Q12** still stand. Q4 (Intel iMac) is answered: it is
    Apple silicon.

---

## Sources (not already inline)

- tart: https://github.com/openai/tart (LICENSE, latest release 2.41.0, 2026-10-13, via
  the GitHub API), `Sources/tart/Commands/Run.swift`, https://tart.run/faq/,
  https://github.com/cirruslabs/macos-image-templates
- Virtualization in macOS 27: https://developer.apple.com/videos/play/wwdc2026/224/,
  https://wwdcnotes.com/documentation/wwdc26-224-expand-the-capabilities-of-your-virtualization-app/
- Unity licensing: https://support.unity.com/hc/en-us/articles/4402049280276,
  https://docs.unity3d.com/ru/2017.3/Manual/ActivationFAQ.html
- Unity Library and Accelerator:
  https://unity.com/blog/engine-platform/tips-for-working-more-effectively-with-the-asset-database
- iMac specs (still M4): https://www.apple.com/imac/specs/
- Xcode RAM: https://mjtsai.com/blog/2022/04/25/how-ram-affects-xcode-compilation-speed/

## Appendix: two details from the second research pass

- **iPhone over USB in a macOS 27 guest: no evidence it works.** macOS 27 adds USB passthrough (`VZUSBPassthroughDeviceConfiguration`, via the Accessory Access framework). It needs the restricted `com.apple.developer.accessory-access.usb` entitlement from a provisioning profile, so it has to run from an app bundle rather than a plain CLI or daemon. In June 2026 a USB drive worked, but an iPhone, a webcam and a mic failed. Apple DTS says there is no list of supported devices (FB23222154): https://developer.apple.com/forums/thread/831902. tart 2.41 has no host-USB passthrough. Keep installs on a real device on a real Mac.
- **Ready-made Xcode guest images** are still published under the cirruslabs org and rebuilt monthly:
  - names: `ghcr.io/cirruslabs/macos-{golden-gate,tahoe,sequoia}-{vanilla,base,xcode:N}`, where "golden-gate" is macOS 27; for example `macos-golden-gate-xcode:27` and `macos-tahoe-xcode:26.5`;
  - base image: a 50 GB disk with tart-guest-agent and a Metal-capabilities shim;
  - xcode image: a 140 GB disk with Xcode, every simulator runtime for that Xcode, fastlane, and the Codex and Claude Code casks;
  - template sources: https://github.com/cirruslabs/macos-image-templates.
  - A golden image for Gravity bots could start from `macos-golden-gate-xcode:27`. Pulls are tens of GB.

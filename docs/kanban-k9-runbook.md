# K9 on real hardware: the owner's run

The 0.19.0 acceptance scenario ([kanban-rework.md](kanban-rework.md), "Acceptance
scenario (K9)") runs automatically on two test daemons:

```sh
cargo test -p hermesd --test integration k9_acceptance::
```

This checklist is for what the test can't reach: the desktop app's UI, the
real phone with K7 ([mrinaldi2/gravity_os#1](https://github.com/mrinaldi2/gravity_os/pull/1),
branch `k7-boards`), real Claude bots, two physical computers, and the
installers. Two known gaps are kept as ignored tests (`k9_gap_*`); they are
marked **G1** and **G2** below, so a hit there is expected, not a new failure.

You need:

- **This Mac**: the board's home, with the desktop app.
- **A linked computer**: the Windows PC, where your Windows bot runs.
- **The phone**, with `k7-boards` installed from Xcode as you install phone
  builds today, paired with this Mac.
- **The branch** `k9-acceptance`. It is stacked on #23 (K8), which holds K4b,
  K5 and K6, so it is the whole 0.19.0 kanban stack so far.

Bots: the scenario reads best with real Claude Code bots, which are billed.
To spend nothing, run the daemon with `runtime = "double"` and act as the bots
with `.claude/skills/bus-live-test/scripts/bus.mjs mcp`.

## 1. Before installing: what you have, and an optional backup

On each computer, note the version you have now:

```sh
~/.thehermes/bin/hermesd --version
```

```powershell
& "$env:USERPROFILE\.thehermes\bin\hermesd.exe" --version
```

The branch migrates the database once, when its daemon starts (boards,
labels, roles). The migration only goes forward, so the daemon backs the
database up first, the same backup `hermesd backup` takes, into
`backups/pre-migration-<UTC time>-schema-<from>-to-<to>` in the home. If that
backup can't be written, the daemon migrates nothing and doesn't start; its
error log says why (free disk space, then start it again). A backup holds the
database and configuration, never `secrets/` or workspaces.

For extra safety you can still take one by hand now, with the version you
have:

```sh
~/.thehermes/bin/hermesd backup        # prints the backup folder; note it
```

```powershell
& "$env:USERPROFILE\.thehermes\bin\hermesd.exe" backup
```

## 2. Build the branch the usual way

The release workflow builds the DMG, the Windows installer and the `hermesd`
tarball. Run by hand on a branch, it only uploads them to the run. It pushes
no tag, makes no GitHub release and publishes no `latest.json`, so no
installed app is offered this build.

```sh
gh workflow run release.yml --ref k9-acceptance
gh run list --workflow release.yml --limit 1      # its id
gh run watch <run-id> --exit-status --interval 30
gh run download <run-id> -n release-macos -n release-windows -D ~/Downloads/k9-build
```

The two artifacts land in `release-macos/` and `release-windows/` under
that folder. The run needs the `HERMES_TEAM_ID` variable that releases
already use.

The branch keeps version 0.18.3 (nothing is bumped until 0.19.0), so an
installed app's updater sees no new version now. It offers 0.19.0 as usual
once that release is published.

Install the DMG only from a run that signed it: the "Enable signing and
notarization" step's log says whether it found the certificate. A daemon
built with your Team ID knows your app by its signature (H-110), so an
ad-hoc signed app may not count as yours, and then it can't act as the owner.

## 3. Install

**Mac.** Quit The Hermes, open the DMG and drag the app over the installed
one. The app's version doesn't change, so install the daemon from the
tarball. This is the release notes' "daemon only" path, and it restarts every
bot on this Mac:

```sh
cd ~/Downloads/k9-build/release-macos
tar -xzf hermesd-0.18.3-aarch64-apple-darwin.tar.gz
hermesd-0.18.3/hermesd service install
shasum -a 256 hermesd-0.18.3/hermesd ~/.thehermes/bin/hermesd   # must match
```

**Windows.** Use the prompt in section 6: your Windows bot compiles, tests
and installs the branch, and that is the condition for merging 0.19.0 into
`main`. To install by hand instead, run
`release-windows/hermes-desktop-0.18.3-x64-setup.exe`. It installs per machine (Windows asks for an admin) and
refreshes the managed daemon even at the same version.

**`hermesd release install`.** Bots use this to deploy an approved release
to their computer. Trying a branch doesn't need it. To exercise it as well,
use a scratch project, not Recipe Box, which must stay free of releases
(step 8). There DevOps publishes the run's files with `hermesd release
publish`, you approve the release, and the tester runs `hermesd release
install <release>` in its session. The swap is rolled back unless the new
service answers with the new build.

**Check the automatic backup.** Once the new daemon is up, there is one
new folder in the home's `backups/`, ending in `-to-<the new schema>`, and
the daemon's log has "backed up the database before migrating it" with its
path:

```sh
ls -d ~/.thehermes/backups/pre-migration-*
grep "before migrating" ~/.thehermes/logs/hermesd.err.log
```

```powershell
Get-ChildItem "$env:USERPROFILE\.thehermes\backups" -Filter "pre-migration-*"
```

Only the newest three pre-migration backups are kept; your own backups are
never removed. Rename one (without the `pre-migration-` prefix) to keep it.

**To go back**, reinstall 0.18.3 from its GitHub release (the DMG and
`hermesd service install` from its tarball, or the Windows installer). Then
restore the pre-migration backup (or your manual one) with that 0.18.3
`hermesd`:

```sh
~/.thehermes/bin/hermesd restore --from <backup folder> --overwrite
~/.thehermes/bin/hermesd service restart
```

Don't run 0.18.3 on the migrated database without restoring it.

**A rehearsal with no risk.** `scripts/dev.sh` in a checkout of the branch
runs its own daemon under `.dev/` on its own port and never touches
`~/.thehermes`. It listens on 127.0.0.1 only, so the phone and the PC can't
reach it. Use it to walk through steps 2–5 on the desktop alone.

## 4. The scenario

Tick each step and note anything that differs.

1. **Project and bots.**
   - [ ] On the Mac, create the project **Recipe Box** with a bot **Lead**,
     and make Lead the project's lead.
   - [ ] On the PC, create **Recipe Box** with a bot **Cook**.
   - [ ] On the Mac, link the project to the PC's. Cook shows on the Mac as
     a linked bot.
   - [ ] Board tab: **Start the board on this computer**. **G1:** the
     project's first board is always the release-gated "Main", prefix RB.
   - [ ] Pair the phone with the Mac (read, control and approve).
2. **The lead's board.**
   - [ ] Ask Lead in chat: "Propose a Simple board named Recipes with key RC."
   - [ ] Needs you shows "Board change proposed for Main: create board
     “Recipes” (simple)", and nothing has changed on the board yet.
   - [ ] Approve it. Lead is told "Approved: create board “Recipes”".
   - [ ] In the board's settings, make Recipes the default and archive Main.
3. **QA by hand.**
   - [ ] In Recipes' settings, add the column **QA** after Doing (active,
     rule "all acceptance criteria checked").
   - [ ] Lead is told "The owner changed board Recipes: add column “QA” …".
   - [ ] Asked to read the board, Lead lists To do, Doing, QA and Done, with
     the key RC.
   - [ ] Ask Lead to add the label group `course: main, dessert`. It applies
     without asking you, and the notice strip shows it.
4. **A draft with a screenshot.**
   - [ ] On the phone, tap **+** on Recipes. Enter the title "Scaling",
     type Feature, label `course:main`, and attach a screenshot.
   - [ ] On the draft, tap **Discuss with a bot**, then Lead. Ask Lead to add
     two acceptance criteria, then to write a short spec as an artifact and
     attach it.
   - [ ] On the desktop, Needs you shows "Lead updated draft RC-…".
   - [ ] The review lists the title, the criteria and the attachment, each by
     Lead.
   - [ ] **Undo** the title. Finalize: the item goes live in To do.
   - [ ] Assign the item to Cook and ask Lead to send Cook a task for it.
     Cook's task lists the screenshot and the spec, and Cook can open both
     from the PC.
5. **The flow.**
   - [ ] Ask Cook to move the item to Doing (it moves), then to QA. QA refuses
     it with "2 acceptance criteria are not checked. (rule.ac_all_checked)".
   - [ ] Ask Lead to check both criteria with evidence. Cook then moves the
     item to QA, then to Done.
   - [ ] Each move shows live on the desktop and on the phone.
6. **Research, from the phone.**
   - [ ] Ask Lead to propose a Simple board named Research with key RS.
   - [ ] On the phone, open Needs you, then **Board change**, and **Approve**
     it (Face ID).
   - [ ] Research appears on the desktop and in the phone's board picker.
7. **The linked computer.**
   - [ ] Ask Lead to create "Compare flours" on Research and assign it to
     Cook. Cook moves it to Doing from the PC, and the Mac shows the move.
   - [ ] Take the Mac off the network (Wi-Fi off).
   - [ ] Asked to read the board, Cook answers as last seen, marked
     read-only.
   - [ ] Asked to move the item, Cook is refused with "The board lives on
     <the Mac's name>, which is unreachable. Try again when it's back."
   - [ ] Bring the Mac back. The PC catches up.
8. **Nothing about this app.** Go through the project and look for
   "desktop", "iOS", "daemon" or "release" in:
   - [ ] both boards, the item drawers, the Drafts tray, Needs you and the
     board settings;
   - [ ] what Lead and Cook were told.

   **G1:** the archived Main is release-gated. **G2:** the bots'
   instructions and their non-board tools still name the daemon and
   releases. Anything else is a finding.

Afterwards, archive Recipe Box, or keep it for the next run.

## 5. Report

Post the ticked list and every difference on #3, with the branch's commit
(`git rev-parse origin/k9-acceptance`), the run id of the build, and the
pre-migration backup folders.

## 6. Prompt for the Windows bot

Paste this into your Windows bot's chat. The install restarts every bot on
that PC, the Windows bot included, so the prompt has it report before
installing and check again once it is back.

```text
Compile, test and install the Gravity 0.19.0 kanban branch `k9-acceptance`
(github.com/mrinaldi2/gravity, stacked on PR #23) on this Windows PC. This is
the rollout condition before release-0.19.0-kanban-rework may merge into
main, so report exactly what happened, failures included.

Rules:
- Never push anything, to any branch: not main, not
  release-0.19.0-kanban-rework, not release/*, not k9-acceptance. No merges,
  no force-push, no tags, no GitHub comments or releases.
- Don't change code to make a check pass. Report the failure instead.
- Work in a fresh worktree in your own workspace, never in another bot's
  checkout. Keep tokens, secrets and personal paths out of the report.

1. `git fetch origin k9-acceptance` and
   `git worktree add <your workspace>\k9 origin/k9-acceptance`. Record
   `git rev-parse HEAD`, `rustc -vV`, `node -v`, `pnpm -v` and
   `python --version`.
2. In the worktree, run the Windows CI checks, saving each command's full
   output to a log file:
     cargo fmt --all --check
     cargo clippy --locked --workspace --all-targets -- -D warnings
     cargo test --locked --workspace --all-targets --no-fail-fast
   Then stage the debug daemon as the native shell's sidecar:
     New-Item -ItemType Directory -Force apps/desktop/src-tauri/binaries
     Copy-Item target/debug/hermesd.exe apps/desktop/src-tauri/binaries/hermesd-x86_64-pc-windows-msvc.exe
   and run cargo fmt --check, cargo clippy -D warnings and cargo test with
   `--manifest-path apps/desktop/src-tauri/Cargo.toml`.
   Also run `cargo test -p hermesd --test integration k9_acceptance::`
   (must pass) and the same with `-- --ignored` (the two k9_gap tests are
   expected to fail; quote their first lines).
   For each failing test, run it alone again. If it still fails, run it on
   origin/main in a second worktree, and say whether main fails it too.
3. Build the installer with `powershell -File scripts/build-nsis.ps1`, the
   script `hermesd release build-installer` runs for Windows releases.
   Record the setup.exe path under
   apps/desktop/src-tauri/target/release/bundle/nsis/ and its SHA-256, and
   the SHA-256 of
   apps/desktop/src-tauri/binaries/hermesd-x86_64-pc-windows-msvc.exe.
   scripts/test-windows-installer.ps1 needs an elevated shell and a PC
   without The Hermes installed. Run it only if both hold; otherwise say
   it wasn't run and why.
4. Before installing, record `hermesd.exe --version`, `service status`, the
   SHA-256 of %USERPROFILE%\.thehermes\bin\hermesd.exe and the number of
   projects and bots. Then take a backup by hand as extra safety (the new
   daemon also takes one before it migrates):
   `& "$env:USERPROFILE\.thehermes\bin\hermesd.exe" backup`.
   Write the results so far to an artifact, send the owner a short summary
   with message_owner, and note in FACTS.md that after the restart you go on
   with step 6.
5. Run the setup.exe (Start-Process -Wait). It installs per machine, so
   Windows asks the owner to allow it. If nobody allows it within 10 minutes,
   tell the owner the installer's path and stop. The install restarts this
   session.
6. Once you are back, check:
   - `hermesd.exe --version` and `service status`;
   - /health answers on the port in %USERPROFILE%\.thehermes\hermesd.port;
   - the installed hermesd.exe hashes to the sidecar from step 3;
   - every project and bot from step 4 is still there;
   - %USERPROFILE%\.thehermes\backups has one new `pre-migration-*`
     folder (record its name), and the daemon's log names it;
   - the desktop app opens and shows the board;
   - `hermesd.exe release install no-such-release --dry-run`, run from your
     session, ends with the daemon's refusal, not a crash.
   If anything is broken, don't try to repair or roll back. Tell the owner,
   with the backup folders from steps 4 and 6.
7. Report to the owner with message_owner and an artifact: the commit,
   the toolchain, each step's result, failing tests with their first error
   lines and whether main fails them too, the installer's path and hash,
   the backup folders, and what step 6 found.
```

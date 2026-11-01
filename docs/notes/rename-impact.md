# Rename impact map: "Gravity / The Hermes" → `<New>`

Research only: no code was changed, nothing pushed, nothing registered.
Code read at:

- `~/Developer/gravity-wt-0.19.0-kanban` (`release-0.19.0-kanban-rework`, 5c7f2543)
- `~/Developer/gravitiOS-wt-k7` (`k7-boards`, 5d42c08)

Below, `<new>` is the lowercase slug (e.g. `hiveyard`), `<New>` the display
name, and `<NEW>_` the env prefix.

## TL;DR

1. **This is the second rename, and the first one left the machinery in place.**
   On 2026-10-04/05, R2 and M1–M5 renamed Gravity to The Hermes. They added:
   - `crates/hermesd/src/brand.rs`, a single module for every name the daemon shows, registers or reads, with `LEGACY_*` twins;
   - `hermesd migrate-home`: a resumable, rollback-able home move with a DB path rewrite, Claude transcript/memory dir renames and worktree repair;
   - the `service install` sequence with `Identity::{Legacy, Current}`;
   - an NSIS hook that takes over the old install;
   - `legacy_storage.rs`, which copies the webview storage;
   - pairing links accepted under both schemes;
   - env vars read with a fallback prefix.

   The cheap path is to **shift that machinery one hop**: today's "current"
   (The Hermes) becomes "legacy" and `<new>` becomes "current". Gravity-era
   compatibility is retired at the same time, once every machine is confirmed
   migrated.
2. **Keep the internal identifiers.** Don't rename these:
   - the proto packages (`hermes.*.v1`);
   - the `hermes/*` JSON-RPC method names;
   - both bundle IDs (`com.manuelrinaldi.thehermes`, `com.manuelrinaldi.gravitiOS`);
   - the crate names;
   - `ON_DISK_SLUG = "gravity"` (worktree markers, `gravity/<bot>` branches).

   Renaming them buys nothing a user sees and risks real breakage: lost Keychain items, lost macOS permission grants, a new App Store record, regenerated code and buf breaking checks.
3. **Effort: about 4–6 bot-days plus one owner-run Windows install.** That
   covers display names in about 1 day, the on-disk and service identity in
   2–3 days including the Windows gate, and repos, site and docs in about 0.5
   day plus the owner's admin work. Most of the cost and risk is in the
   on-disk and service identity. Display names alone are about one day and
   carry low risk.

## Hit counts (`git grep -i -c`)

| pattern | desktop/daemon repo | iOS repo |
|---|---|---|
| `gravity` | 921 hits / 238 files | 360 / 89 |
| `gravit` | 933 / 238 | 390 / 94 |
| `hermes` | 3981 / 704 | 2115 / 156 |
| `thehermes` | 276 / 90 | 153 / 63 |
| `hermesd` | 1407 / 352 | 25 / 16 |

Where the desktop/daemon hits (`hermes|gravit`) sit:

| area | hits |
|---|---|
| `apps/desktop` | 2029 |
| `crates/hermesd` | 1900 |
| `crates/bus` | 137 |
| `.hermes/reviewers.toml` | 97 |
| `docs/protocol.md` | 76 |
| `.agents/skills` + `.claude/skills` (mirrors) | 64 + 63 |
| `.github/workflows` | 53 |
| `apps/marketing` | 50 |
| `README.md` | 44 |
| `proto/hermes` | 31 |
| `scripts/*` | ~60 |

Most of the `hermes` count is the CLI name `hermesd` and generated proto code.

**User-visible words**, counted as `git grep -w`:

- Desktop UI (`apps/desktop/src`, excluding tests and generated code): "Hermes" 129 times in 55 files, plus 101 in tests. "Gravity" appears only twice.
- Daemon crates: "Hermes" 151 times, "Gravity" 29 times (mostly legacy constants and comments).
- iOS Swift: "Hermes" 191 times, "Gravity" 35 times. Non-Swift files: "Gravity" 93 times (README, the Gravity Lens companion), "GravitiOS" 21 times.
- iOS target, module and folder names `TheHermes` / `TheHermes.xcodeproj`: 109 references in 49 files.

## Groups

Each group below lists what exists, where, the risk, and the migration needed for existing installs.

### 1. User-visible names and strings: rename

- **Daemon:** `brand::DISPLAY_NAME` ("The Hermes") and `brand::SHORT_NAME` ("Hermes"). They feed:
  - the bot system prompt (`paths/prompt.rs:60`, `prompt_sections.rs:123,250`);
  - CLI messages ("Asking for your OK in The Hermes app…");
  - refusal texts listed in `docs/protocol.md`;
  - the worker git author (`WORKER_GIT_SUFFIX = "(Hermes worker)"`, `worker@thehermes.invalid`).

  About 86 literal "The Hermes" strings in `crates/` (tests included) don't go through `brand`. That includes `bus_auth/app_identity.rs` and its tests.
- **Desktop:**
  - `tauri.conf.json` and `tauri.windows.conf.json`: `productName`, window `title`;
  - `Info.plist` usage strings ("Hermes listens while you dictate…");
  - about 37 literal "The Hermes" strings in `apps/desktop/src` (e.g. `analytics.tsx:240`, "The Hermes stopped unexpectedly"), plus the Rust side (`src-tauri/src`).
- **iOS:**
  - `INFOPLIST_KEY_CFBundleDisplayName = "The Hermes"` (pbxproj lines 374 and 400);
  - the `NS*UsageDescription` strings (Face ID, camera, local network);
  - about 191 "Hermes" strings in Swift;
  - README and screenshots say "GravitiOS";
  - the issue templates mention "Gravity Lens".
- **Risk: low.** This is mechanical, but many tests assert on literal strings: about 101 desktop test hits and the `crates/hermesd/tests/branding.rs` tests. Route the remaining literals through `brand` (daemon) and one constant (desktop and iOS) first, so the rename is a one-line change plus a test sweep.
- **Migration:** none for installs.
  - Bots' `system.md` is regenerated at every boot from `brand`.
  - Old transcripts and auto-memories keep saying "Hermes". That's harmless. Mention it in the release notes, because bots will remember the old name for a while.
  - The `.app` name follows `productName`. See group 2 for the side effects.

### 2. Bundle IDs, app identifiers, URL schemes: keep the IDs, add a scheme

| item | where | recommendation |
|---|---|---|
| macOS/Windows app identifier `com.manuelrinaldi.thehermes` | `tauri.conf.json:5`, `bus_auth/app_identity.rs:20` (`BUNDLE_ID`), `legacy_storage.rs:17`, `docs/protocol.md:499` code requirement `anchor apple generic and identifier "com.manuelrinaldi.thehermes" and certificate leaf[subject.OU] = "<Team ID>"` | **Keep.** It's invisible to users. Changing it again would mean: (a) the daemon's compiled owner identity changes, so for the transition `app_identity` must accept both identifiers, or the new app can't get an owner ticket from the old daemon (and the reverse after a downgrade); (b) webview localStorage needs another `legacy_storage` copy; (c) macOS TCC grants (microphone, speech, accessibility for the global shortcut) are keyed by identifier and **can't** be carried over, so the owner is asked again on every Mac. The previous rename already paid this cost once. |
| iOS bundle ID `com.manuelrinaldi.gravitiOS` | set only in the git-ignored `Config/Local.xcconfig` (the tracked `Signing.xcconfig` has `com.example.gravitios`) | **Keep.** `Keychain.swift:6` derives its service from the bundle ID (`"<bundleId>.daemon"`), so a new ID silently **drops every paired computer's token** on the phone. It would also be a new App Store/TestFlight app with no update path from the old one. Change only the display name. |
| iOS pairing URL schemes `thehermes://`, `gravity://` | `Config/Info.plist` `CFBundleURLSchemes`, `Pairing.swift:8` (`schemes = ["thehermes", "gravity"]`), `CardLinks.swift:12` (`hermes`, `thehermes`) | **Add `<new>://`, keep accepting the old schemes.** Order matters: ship the phone build that accepts `<new>://` **before** the desktop starts emitting it in pairing QR codes and card links. Otherwise an old phone can't scan a new code. |
| Windows trusted-app folder `Program Files\The Hermes` | `app_identity.rs` (`WINDOWS_APP_FOLDER`, around lines 112–118), NSIS `perMachine` install dir = `productName` | Changes automatically with `productName`. The daemon and the app ship together, so the new daemon trusts `Program Files\<New>`. During the swap, the NSIS hook must take over the old `The Hermes` folder the way `GRAVITY_TAKE_OVER_LEGACY_INSTALL` already handles "Gravity". Otherwise the old folder's exe stays installed but is no longer trusted. |
| macOS `/Applications/The Hermes.app` | follows `productName`; hard-coded in `bot_permissions/exact.rs:92,112` (an exact-command allowlist for `"/Applications/The Hermes.app/Contents/MacOS/hermesd" release install <id>`), `service_audit.rs`, `releases/install/rollback_notice.rs`, tests | The new DMG installs `/Applications/<New>.app` **next to** the old one, and both carry the same bundle ID, which confuses LaunchServices about which app opens. `release install` must replace or remove the old `.app`. Update the exact-command allowlist to accept both paths for one release, or bots' `release install` calls start asking for permission. |

### 3. launchd label and Windows task: rename only together with the binary

- **macOS label:** `com.manuelrinaldi.thehermesd`, in `brand.rs:63`, `desktop daemon/home.rs:13`, `ops/com.manuelrinaldi.thehermesd.plist`, `service/tests` and README. `LEGACY_LAUNCHD_LABEL = "in.mikolajczuk.gravityd"` is still in place.
- **Windows:** a per-user scheduled task named `The Hermes-<sid>-<hash>` (`brand::WINDOWS_TASK`); the legacy name is `Gravity`. There is also the named pipe `\\.\pipe\thehermes-<sid>-<home hash>-bus` (`bus_auth/ipc/windows.rs:103`).
- **Risk: medium.** `service/sequence.rs` models exactly **one** legacy identity (`Identity::Legacy`). Two options:
  - **(a) Shift the hop.** Recommended if every machine is already on `.thehermes` and has `meta.home_migrated`. Set `LEGACY_LAUNCHD_LABEL` to `com.manuelrinaldi.thehermesd`, `LEGACY_WINDOWS_TASK` to `The Hermes`, and `LAUNCHD_LABEL` to `com.manuelrinaldi.<new>d`. Delete the Gravity constants.
  - **(b) Make Legacy a list**, for a machine that might still be on Gravity. This costs more and nobody probably needs it.

  The sequence already handles stop old → migrate → start new → remove old only after `/health` reports the new version, with rollback, so the order is safe.
- **Pipe name:** it's re-derived at every start and clients read `<home>/run/endpoint`, so rename it freely. Nothing persists.
- **Migration:** `service install` from the new build unloads the old label or task first. That is the existing behavior once the constants have moved. The desktop's mirrored constants in `apps/desktop/src-tauri/src/daemon/home.rs` must change in the same release; its comment says "Must match".

### 4. Binary names, sidecar, installers: rename `hermesd` if the CLI is user-facing, keep `hermes-desktop`

- **The daemon/CLI `hermesd` is the most visible internal name.** The owner and bots type it:
  - bot prompts tell bots to run `hermesd pr merge` (`prompt_sections.rs:204`);
  - the bus is `hermesd bus-proxy` in every `mcp.json`;
  - the hooks are `hermesd hook <event>`;
  - files in the home are named `hermesd.toml`, `logs/hermesd.*.log` and `hermesd-task.*`.

  `DAEMON_FILE_STEM` and the migrate-home "rename daemon files" step already parameterize the home files.
- **Sidecar:** `externalBin: ["binaries/hermesd"]`, `scripts/prepare-sidecar.{sh,ps1}` (`hermesd-<triple>`), NSIS hooks (`$INSTDIR\hermesd.exe`, `.old` swap), and the `release.yml` assets:
  - `hermesd-$VERSION-aarch64-apple-darwin.tar.gz`
  - `hermes-desktop-$VERSION-aarch64.dmg`
  - `hermes-desktop-$VERSION-x64-setup.exe`
  - `.app.tar.gz`
- **`hermes-desktop`** is the Cargo package name. It is the exe name inside the `.app` and in `Program Files`, and it shows in Task Manager and Activity Monitor. **Keep the crate name.** Optionally set Tauri `mainBinaryName` or `[[bin]] name` to `<new>` for a cleaner process name. The Windows trust check accepts any exe in the folder except `hermesd`, so update that exclusion to the new daemon name.
- **Risk: medium.** Bots' existing `mcp.json` and Codex configs point at `hermesd bus-proxy`. They are regenerated at each bot start, but running sessions keep the old path until restarted.
- **Migration:**
  - Ship a `hermesd` → `<new>d` compatibility shim for one release, as a symlink in the home `bin/` and a copy on Windows. Then live sessions and the owner's muscle memory keep working.
  - Rename the files in the home through the existing "rename daemon files" migrate-home step, with `LEGACY_DAEMON_FILE_STEM = "hermesd"`.
  - Asset names in `release.yml` can change freely: nothing fetches them by name except the updater manifest (group 7) and `apps/marketing`'s `latest.json` bridge.

### 5. Home folder and env vars: rename with the existing migration, keep a fallback

- **Home folder:** `~/.thehermes` (`brand::HOME_DIR_NAME`), dev home `~/.thehermes-dev` (`config/overridden.rs:130`). `LEGACY_HOME_DIR_NAME = ".gravity"`.
- **There is a home-migration feature: `hermesd migrate-home`** (`crates/hermesd/src/migrate_home.rs` plus 11 files, about 2,300 lines). Each step is recorded in `migrate-home.json`, it can resume after a crash, and it supports `--rollback`. The steps:
  1. back up the DB, config and transcript dirs;
  2. move the home;
  3. rename the daemon files;
  4. rewrite home paths in the database in one transaction (`meta.home_migrated`, `meta.home_migrated_from`), in `hermesd.toml`, and in the files `claude_args` names;
  5. rename the Claude Code transcript and memory dirs, which are keyed by the mangled workspace path, so every bot keeps `--continue`, chat history and auto-memory;
  6. symlink (on Windows, junction) the old path to the new home for one release;
  7. repair git worktrees that recorded the old path.

  It also handles path spellings: `/var` vs `/private/var`, and MSYS `/c/Users/...` on Windows. `Plan::new(from, to, user_home)` already takes arbitrary paths. Only `Plan::default_for` hard-codes `LEGACY_HOME_DIR_NAME`.
- **What it needs for hop 2:**
  - Point `LEGACY_HOME_DIR_NAME` at `.thehermes` (option (a) above).
  - `meta.home_migrated` is a single flag already set by hop 1. Hop 2 needs a new key, or must compare `home_migrated_from` with the current legacy, or the migration will think it's already done. **This needs code; check it first.**
  - The NSIS hook's pre-check (`installer-hooks.nsh:202-204`: "if `.gravity\bus.sqlite` exists and `.thehermes` doesn't, ask to move") needs the new pair.
  - The desktop's `daemon_home()` fallback (`daemon/home.rs`) must read both layouts on the first launch after the update, as it does today.
- **Latent gap found: the migration doesn't touch the macOS Keychain.** `secrets/vault.rs:122-126` names the peer-link service `"The Hermes peer link <sha256(secrets_dir)[..12]>"`. The hash is of the **secrets path**, so any home move changes it. When `[auth] peer_tokens = "keychain"` (opt-in, macOS), every linked computer's token is orphaned after the move. Hop 1 had the same gap; it probably didn't bite because the default is `file`. Hop 2 should either re-key the items in a migrate step, or pin the service name to a home ID stored in the DB instead of the path. Check whether any machine uses `peer_tokens = "keychain"`.
- **Env vars** (occurrences in the repo):

  | variable | count | variable | count |
  |---|---|---|---|
  | `GRAVITY_HOME` | 75 | `THEHERMES_SCRATCH` | 5 |
  | `HERMES_DEV_BUILD` | 23 | `GRAVITY_RUNTIME` | 5 |
  | `THEHERMES_HOME` | 22 | `GRAVITY_DEV_PORT` | 8 |
  | `HERMES_TEAM_ID` | 21 | `GRAVITY_DEV_TOKEN` | 7 |
  | `GRAVITY_WS` | 16 | `HERMES_RELEASE_BUILD` | 7 |
  | `THEHERMES_TOKEN` | 12 | test-only `HERMES_*` | (several) |
  | `GRAVITY_TOKEN` | 9 | `GRAVITY_*` NSIS macro names | (several) |
  | `THEHERMES_SESSION` | 6 | | |

  - **Runtime vars** (`THEHERMES_*`, with `GRAVITY_*` as fallback): `brand::env_var_os` reads the `ENV_PREFIX`, then the `LEGACY_ENV_PREFIX`. Bots get both token names (`bot_token_vars`). Shift the hop: `<NEW>_*` first, then `THEHERMES_*`. Keep `GRAVITY_*` only if (b). The desktop has its own `["THEHERMES_HOME", "GRAVITY_HOME"]` list (`daemon/home.rs`). Bot prompts mention `$THEHERMES_SCRATCH`, so set both for one release.
  - **Build-time vars** (`HERMES_TEAM_ID`, `HERMES_DEV_BUILD`, `HERMES_RELEASE_BUILD`, `HERMES_WINDOWS_SIGNER`): **keep.** Only CI and scripts read them. Renaming touches `release.yml`, `prepare-sidecar.*`, `scripts/dev.sh` and `build-nsis.ps1` for no user value.
  - Many `GRAVITY_DEV_*` and `GRAVITY_WS` names are dev-script internals. Leave them, or sweep them in a later cleanup.
- **Repo-level policy folder `.hermes/`** (`prs/policy.rs:18-22`: `.hermes/reviewers.toml`, `.hermes/checks.toml`, `POLICY_PATHS = [".hermes/*.toml"]`) lives **inside every project repo**, not the home.
  - If it's renamed, read `.<new>/` first, then `.hermes/`.
  - **Security:** `POLICY_PATHS` must protect **both** globs during the transition. Otherwise a bot can add `.<new>/reviewers.toml` and change review policy without the owner's review, because only `.hermes/*.toml` is owner-reviewed (`prs/owner.rs:71`).
  - Simplest is to **keep `.hermes/`**. It's a config folder, like `.github/`.

### 6. Proto packages `hermes.{board,home,pr,wire}.v1`: keep

- **Sizes:**
  - 4 packages under `proto/hermes/*/v1`;
  - the iOS copy under `contract/proto` with `SOURCE`;
  - TS generated into `apps/desktop/src/protocol/gen/hermes/...`;
  - Rust generated at build time (`crates/bus/build_schema.rs:15`, `PACKAGES = ["hermes.board.v1", "hermes.pr.v1"]`);
  - Swift generated `Hermes_Board_V1_*` types: **1,629 references in 36 Swift files**.
- **Wire effect:**
  - Messages are binary protobuf (`DaemonClient.swift:66`: "typed `hermes.wire.v1.Envelope`s").
  - No `google.protobuf.Any` or type URLs were found, so the package name **isn't in the bytes**. A rename wouldn't strictly break old and new peers on the wire.
  - It is still a contract break. `buf breaking` (`FILE`) fails; every generated symbol changes in three languages; fixtures and schema tooling key on the full names (`crates/bus/fixtures/*`, `contracts.test.ts`); and the phone and desktop contract copies must change in lock-step.
- **Recommendation: keep `hermes.*.v1` for good**, as a frozen codename, like `ON_DISK_SLUG = "gravity"` was kept last time. If a v2 package is ever cut for other reasons, name it then.
- **Same answer for the JSON-RPC methods** `hermes/owner_ticket`, `hermes/owner_request`, `hermes/pr_merge`, `hermes/release_*`, `hermes/leave_out*`, `hermes/permission`, `hermes/hook`. These names *are* on the wire, between CLI, app and daemon, and possibly between versions on linked computers. Keep them.
- **MCP bus server name `hermes-bus`:**
  - Bots see it, the owner sees it in Claude Code permission prompts, and transcripts record `mcp__hermes-bus__*`.
  - `brand::BUS_TOOL_PREFIXES` already accepts two prefixes, so renaming means adding a third, `mcp__<new>-bus__`.
  - Bot permission allowlists also name it, so they must cover both.
  - Optional; low value. I'd keep `hermes-bus` unless the owner dislikes seeing it.

### 7. Crate names, updater, GitHub repos

- **Crates** `hermesd`, `bus`, `hermes-desktop` / `hermes_desktop_lib`, and npm packages `the-hermes` (root), `hermes-desktop`, `hermes-marketing`: **keep.** The crate name only matters through the binary name, and `[[bin]] name = "<new>d"` decouples them. Renaming the crates means Cargo.lock churn, every `-p hermesd` in scripts and CI, and build caches. If the CLI is renamed, rename just the bin.
- **Updater:**
  - `tauri.conf.json` ships `updater.endpoints: []`, `pubkey: ""`, `createUpdaterArtifacts: false`.
  - `release.yml:57-74` turns it on only when `TAURI_SIGNING_PRIVATE_KEY` and `vars.UPDATER_PUBLIC_KEY` are set. The endpoint is then derived as `https://github.com/$GITHUB_REPOSITORY/releases/latest/download/latest.json`.
  - The repo shows **no GitHub releases and no Actions variables** (checked with `gh api`, 2026-10-14). The updater feed doesn't look live, and installs come from `hermesd release install` or a local DMG/NSIS.
  - Continuity is therefore low-risk today. If it's ever enabled, **keep the same signing keypair** across the rename (the public key is compiled into installed apps). Note that the endpoint embeds the repo name: GitHub redirects release-download URLs after a repo rename, but only until someone creates a repo with the old name. Prefer a stable endpoint you own (the marketing worker already has a `/desktop/gravity/latest.json` bridge, `apps/marketing/README.md:31`; rename that path with a redirect).
- **GitHub repos** `mrinaldi2/gravity` (public, homepage `https://getgravity.build`) and `mrinaldi2/gravity_os` (public):
  - Renaming is an owner action in repo settings. GitHub redirects web, git and API URLs from the old name.
  - Afterwards, update: bots' clone remotes (they keep working through the redirect, but update them); any project rows in the daemon DB that store the repo slug or URL; `release.yml` mentions; README badges; `gravity_os` docs pointing at the desktop repo (`contract/SOURCE`).
  - **Don't recreate a repo under the old name**, which breaks the redirects.
- **Domain:** `getgravity.build` (Cloudflare, created 2026-09-02, expires 2027-09-02; owner not verified). Keep it pointing at the new site for a while.

### 8. Bot prompts that name the product

- `paths/prompt.rs` and `prompt_sections.rs` pull `brand::SHORT_NAME` and `ACTIVE_MCP_SERVER`. Literal mentions are `$THEHERMES_SCRATCH`, `hermesd pr merge` and the `gravity/<bot>` branch examples. That last one is `ON_DISK_SLUG`; keep it.
- `db/typed_prompts.rs` and the `.hermes/reviewers.toml` role texts (97 hits) name the product too.
- **Migration:** `system.md` is regenerated for every bot at boot, so new sessions pick up the new name automatically. Existing auto-memory and `CLAUDE.md`-style notes inside bot workspaces still say Hermes or Gravity. Either leave them, or have a one-off routine ask bots to update their memory.

### 9. Skills, docs, marketing, telemetry

- **Agent skills:** `.claude/skills/*` and the `.agents/skills/*` mirror: `release`, `bus-live-test`, `marketing-screenshot`, `terminal-perf-test`, `visual-regression`, `lib`. The skill **names** are product-neutral. Only their descriptions and text mention Gravity or `hermesd`. **No Claude Code plugin or marketplace manifest exists.** Low risk: a text sweep.
- **Docs:** `docs/` (~200 hits, `protocol.md` 76, `architecture.md`, `windows.md`, `peer-bots.md`, the kanban runbooks), README (44), CONTRIBUTING, SECURITY, `release-notes-template.md`, `third-party/*NOTICES*` (3 each, regenerated by `scripts/notices.py`). This is a text sweep, and it's also where to document the old-name compatibility.
- **Marketing** (`apps/marketing`, 50 hits) is **still branded "Gravity"**: `<title>Gravity — a team of persistent bots for macOS</title>`, OG tags pointing at `https://getgravity.build`, and the updater bridge route `/desktop/gravity/latest.json`. The previous rename never reached it. Redo it fully with the new name. Keep the old route answering with a redirect if the updater is ever enabled.
- **Telemetry:** PostHog (`apps/desktop/src/analytics.tsx`, `VITE_POSTHOG_HOST=https://eu.i.posthog.com`, project token injected at build). Event names (`app_launched`, …) carry no product name. Renaming the PostHog project is cosmetic and is done on the owner's side. Low risk.
- **iOS companion "Gravity Lens"** (`companion/`):
  - macOS launchd label `gravitios.gravity-lens` and `gravitios.autostart`, home `~/.gravity-lens`;
  - Windows scheduled task "Gravity Lens", folder `%USERPROFILE%\.gravity-lens`;
  - files `gravity_lens.py` and `gravity_files.py`.

  It's installed on linked computers by `install.sh` / `install.ps1`. If it's renamed, the installers must unload and remove the old label or task first. Otherwise two copies run and fight over the port.

### 10. Paired phones and linked computers

- **Paired phones:**
  - The phone keeps each computer's host and device token in its Keychain under `"<bundle id>.daemon"`. This survives as long as the iOS bundle ID is kept.
  - The daemon side stores devices as `secrets/device-<id>.sha256` in the home, which moves with migrate-home.
  - The address and port are unchanged.

  **Nothing to re-pair**, provided the iOS bundle ID is kept and the URL-scheme order in group 2 is followed.
- **Linked computers (peers):**
  - Peer tokens live in the home's `secrets/` (default), so they move with the home.
  - With `peer_tokens = "keychain"` they are orphaned by the path-hash service name (group 5). **Fix this before hop 2.**
  - The peer protocol uses no product-named headers or schemes; the only product-named RPC methods are the `hermes/*` ones, which are kept.
- **Mixed versions:** each machine migrates independently on its own `service install`. Because the wire names are kept, an un-updated peer keeps talking to an updated one. Roll out machine by machine. The **Windows bot machine goes last** and gates the merge, per the 0.19.0 constraint.

## What NOT to rename (cost > value)

| identifier | why keep |
|---|---|
| `com.manuelrinaldi.thehermes` (macOS app ID) | TCC re-prompts on every Mac, a webview storage copy, owner-identity code requirement churn; invisible to users |
| `com.manuelrinaldi.gravitiOS` (iOS bundle ID) | it's a new App Store app and paired-computer Keychain entries are lost |
| `hermes.{board,home,pr,wire}.v1` proto packages | 1,629 Swift refs plus TS/Rust codegen, buf breaking, lock-step phone/desktop contracts; nobody sees them |
| `hermes/*` JSON-RPC methods | on the wire between versions and machines |
| `ON_DISK_SLUG = "gravity"` (`.gravity-worktree.json`, `gravity/<bot>` branches) | lives in bots' repos outside the home; it was kept deliberately last time for the same reason |
| crate and npm package names (`hermesd`, `bus`, `hermes-desktop`, `the-hermes`, …) | use `[[bin]] name` / `mainBinaryName` instead |
| build-time env vars `HERMES_TEAM_ID`, `HERMES_DEV_BUILD`, `HERMES_RELEASE_BUILD`, `HERMES_WINDOWS_SIGNER` | CI and scripts only |
| `hermes-bus` MCP server name | optional; the prefix list handles a rename, but it means a third prefix for little value |
| `.hermes/` repo policy folder | optional; if renamed, both paths must be owner-protected |
| iOS target, module and folder `TheHermes` / `TheHermes.xcodeproj` | an Xcode project rename is churny (schemes, test targets, `@testable import TheHermes`) and invisible in the product |

## Proposed order of work

0. **Owner decisions and admin** (no code):
   - pick the name and get a trademark knockout;
   - grab the domain(s) and GitHub org or handle (if wanted);
   - decide option (a) "shift the hop" vs (b) "legacy list";
   - confirm every machine (Mac, Windows bot, any Linux) runs `.thehermes` with `meta.home_migrated` set, and whether any uses `peer_tokens = "keychain"`.
1. **R0, centralize names (0.5 day, no visible change).**
   - Daemon: route the ~86 remaining crate literals through `brand`.
   - Desktop: add one `brand.ts` / Rust const for the display name and import it everywhere.
   - iOS: add one `Brand.swift`.
   - Make tests assert against the constants, not literals.

   This makes every later step a small diff, and it can land before the name is chosen.
2. **R1, display name (0.5–1 day).** `productName`, window titles, `Info.plist` strings, `DISPLAY_NAME` / `SHORT_NAME`, iOS `CFBundleDisplayName` and usage strings, refusal texts, `WORKER_GIT_*`. The **Windows folder and `.app` name change here**, so include:
   - the NSIS takeover of `Program Files\The Hermes` (copy the `GRAVITY_TAKE_OVER_LEGACY_INSTALL` macro with a `LEGACY_PRODUCTNAME "The Hermes"`);
   - `release install` removing `/Applications/The Hermes.app`;
   - the exact-command allowlist accepting both app paths.
3. **iOS URL scheme (0.25 day).** Accept `<new>://`, keep `thehermes` and `gravity`. Ship the phone build. Only after that, step 4's desktop emits `<new>://` QR codes.
4. **R2, on-disk and service identity (2–3 days, the risky part).** Shift `brand.rs` one hop:
   - `HOME_DIR_NAME`, `DAEMON_FILE_STEM`, `LAUNCHD_LABEL`, `WINDOWS_TASK`, `ENV_PREFIX`, `BOT_TOKEN_ENV`, with the old values becoming `LEGACY_*`;
   - the pipe name;
   - the `[[bin]] name = "<new>d"` plus a `hermesd` shim for one release;
   - the sidecar names and `release.yml` asset names;
   - the `meta.home_migrated` "which hop" fix;
   - the Keychain re-key step;
   - the NSIS home pre-check;
   - the desktop's mirrored constants in `daemon/home.rs`;
   - bot prompt literals (`$THEHERMES_SCRATCH`, `hermesd pr merge`).

   Run the full migrate-home test suite (`resume_tests`, `rollback`, `env_home_tests`, `launchd_premigrate_tests`, `sequence_premigrate_tests`) with hop-2 fixtures. **Gate:** a real install on the Mac, then on the Windows bot machine (owner-run, as for 0.19.0), then a rollback rehearsal on one of them.
5. **R3, repos, site, docs (0.5 day plus owner clicks).** The owner renames the GitHub repos; then update remotes, `contract/SOURCE`, README, docs and skills text, the `apps/marketing` rebrand plus the new domain, the `latest.json` route redirect, the Gravity Lens companion (optional), and the PostHog project name.
6. **R4, cleanup one release later.** Drop the `hermesd` shim, the `~/.thehermes` symlink or junction (migrate-home's "link for one release"), the `THEHERMES_*` dual-setting for bots, and the old URL schemes if desired.

**Total: about 4–6 bot-days** plus the owner's Windows run. For calibration, hop 1 (Gravity to The Hermes) was 25 commits on the brand, migrate-home, service-sequence, legacy-storage and NSIS files over about 2 days (Oct 4–5); R2 alone touched 65 files (+2,433/−370), followed by five migrate-home fix rounds (M1–M5). Hop 2 reuses all of that, so the new code is mostly constants, the `home_migrated` hop fix and the Keychain re-key. Most of the time goes to tests and the real-install gates.

**Display-name-only alternative (~1 day, low risk):** do R0, R1 and the marketing part of R3, and keep `hermesd`, `~/.thehermes` and the labels as internal codenames. Many products ship like this. The visible leftovers are the CLI name, the home folder and the env vars, which power users and bots see.

## Open questions for the owner

1. Is every install already on `~/.thehermes`, with no machine still on `~/.gravity`? This decides option (a) or (b).
2. Does any machine use `[auth] peer_tokens = "keychain"`? If so, the Keychain re-key is a blocker.
3. Should the CLI be renamed (`<new>d`), or should `hermesd` stay as a codename?
4. Should the `.hermes/` repo policy folder and the `hermes-bus` MCP name be renamed, or kept as codenames?
5. Is the updater going to be enabled? If yes, set a stable owned endpoint before or with the rename, and keep the signing keypair.

# Name shortlist (research for the rename decision)

Research only. Nothing was registered, bought or submitted. All checks were
run on 2026-10-14.

## How each check was done, and how far to trust it

| check | method | reliability |
|---|---|---|
| Domains | RDAP lookups straight to the registries: Verisign for `.com`, Google Registry for `.dev`/`.app`, and `whois.nic.ai` for `.ai`. 404 means **not registered**; 200 means **registered** (possibly parked, not necessarily in use). | Good signal. No price or premium info (a premium-priced domain shows as "free"). Registrar pages were not used. |
| GitHub | `github.com/<name>` status, plus GitHub repository search by name sorted by stars (`gh api search/repositories`). | Good. |
| App Store | iTunes Search API, US store, `entity=software` and `macSoftware`, counting apps whose title contains the name. | Good for the US store. Other countries were not checked. |
| Package registries | crates.io, npm, PyPI and Homebrew, for finalists only. | Good. |
| Existing software / AI products | Web searches, 2–4 queries per name (`<name> software`, `<name> AI agent`, `<name> developer tool`), done by three research sub-agents plus my own spot checks. | Medium. Small or new projects can be missed. |
| Trademarks | Trademarkia's US search pages, an unofficial USPTO mirror of unknown freshness. **USPTO, EUIPO and WIPO were not reachable in a usable way.** WIPO Brand DB is behind a human-verification gate, which was not bypassed. Justia returned 403. USPTO tmsearch and EUIPO are JavaScript-heavy. | **US only, unverified.** Nothing below is a clearance search. Before committing, run a real USPTO and EUIPO knockout in classes 9 and 42, ideally with an attorney. |

**One finding across all names:** every real-word `.com` checked is registered,
all 70+ of them, including obscure words like `hiveyard.com` and
`botswain.com`. Plan on `<name>.dev`, `<name>.app` or `get<name>.com` (the
pattern of the current `getgravity.build`) rather than a bare `.com`.

## About 60 candidates by style

The list started at 40 and grew while screening.

- **Mythic and Greek** (continuity with "Hermes"): Daedalus, Kairos, Orrery, Golem, Argonaut, Janus, Ariadne, Theseus, Talos, Mimir, Huginn, Brokkr, Sindri, Dvalin, Lemnos, Telchine, Ergane, Petasos, Cyllene, Myrmex, Myrmidon
- **Crew, ship and fleet:** Bosun, Botswain, Coxswain, Muster, Flotilla, Quarterdeck, Foredeck, Wardroom, Wheelhouse, Pilothouse, Shipwright, Drydock, Galley, Keelson, Halyard, Ketch, Brigantine, Stevedore, Crewdeck, Shipmate, Crewmate
- **Hive, flock and guild:** Hiveyard, Hivewright, Hivecraft, Rookery, Apiary, Covey, Starling, Workhive, Guildry, Cadre, Brigade, Conclave, Althing, Thingstead, Hird, Fyrd, Comitatus, Mesnie, Longhouse, Bothy, Crewhaus, Crewyard, Kith
- **Short invented words:** Tessel, Corvo, Ovrin, Novik, Brivo, Quorra, Velto, Kestro, Tillr, Crewly, Kanbot, Botwright, Botyard, Tinkerage, Crewfold, Quaywork, Keelwork
- **Verbs:** Muster, Wrangle, Marshal, Steward, Kindle, Rally, Tend, Tinker

Many were dropped after the domain, GitHub and App Store screen alone. For
example:

- **Talos:** Talos Linux.
- **Kairos:** a Kubernetes OS.
- **Mimir:** Grafana Mimir.
- **Huginn:** the well-known "agents" automation project.
- **Golem:** the Golem Network.
- **Apiary:** Oracle Apiary.
- **Drydock** and **Shipwright:** existing DevOps tools.
- **Starling:** Starling Bank.
- **Fyrd** and **Lemnos:** the GitHub handle belongs to a well-known developer.
- **Petasos:** the App Store already has *"Petasos: Agent Cockpit"* in Developer Tools.

## The 12 shortlisted names, checked in depth

Some eliminated names were checked too, because they would otherwise have been
obvious picks.

Legend for the domain columns: **free** = not registered at the registry
(RDAP 404); taken = registered.

| # | name | idea | `.com` | `.dev` | `.app` | other domains | github.com/name | App Store (US) | Software / AI conflicts | US TM (Trademarkia, unverified) | Conflict |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | **Hiveyard** | A yard of hives. Each bot has its own hive (its own VM), and together they make the honey. | taken (a small Webflow design studio's site) | **free** | **free** | `gethiveyard.com`, `hiveyardhq.com`, `hiveyard.ai` free | **free** (404) | 0 title matches | No product found in 4 searches. Two 0★ repos (a beekeeping desk, a privacy-policy page for an app called HiveYard). The "Hive" prefix is busy: hive.com (project management), the aden-hive multi-agent harness, Hivemind. | 0 results | **Low** |
| 2 | **Telchine** | The Telchines: the mythical smith-sorcerers of Rhodes, the first metalworkers. Crafty makers, Greek like Hermes. | taken (no site) | **free** | **free** | `gettelchine.com` free; `telchine.ai` taken (2024, Gandi) | **free** | 0 | None found. Two 0★ repos: `ivanbard/telchines-harness` ("LLM harness … for hardware engineering") and a hardware verification toolkit. | 0 results | **Low** |
| 3 | **Botswain** | A pun on boatswain/bosun, the officer who runs the deck crew. You're the captain; the bots are the crew. | taken (no site) | **free** | **free** | `getbotswain.com`, `botswainhq.com`, `botswain.ai` free | user exists (0 repos) | 0 | Nothing with the exact name. A 0★ "verifiable-reasoning ledger for AI agents" repo and a dice Discord bot. Near-sound-alike: *Boatswain* (Stream Deck app for Linux). | 0 results | **Low** |
| 4 | **Wardroom** | The officers' mess, where a ship's officers meet and decide. Fits meetings and approvals. | taken | taken | taken | `getwardroom.com`, `wardroomhq.com`, `.ai` taken | **free** | 1 (a medical app) | `vmware-archive/wardroom` (164★, archived Kubernetes image tool). A 1★ "local-first orchestrator" for two Claude roles. "War room" is crowded in agent tooling (AgentsRoom markets an "AI coding war room"). npm and PyPI names are taken. | Only WARDROOM COFFEE COMPANY (class 25) | **Low–Medium** |
| 5 | **Flotilla** | A fleet of small ships: each bot on its own computer, sailing together. Fits linked computers and per-bot VMs. | taken | taken | taken | all variants taken | org exists (3 repos) | 10 (yacht charters, Unraid/Proxmox, chat) | Stitch Fix `flotilla-os` (195★, job runner). Small agent projects: a fleet-management MCP server, an agent-services Python framework. crates, npm and PyPI names taken. | No live class 9/42 marks. Crusoe Energy's FLOTILLA class 42 cloud filing was abandoned in Apr 2026. PHLOTILLA (boat SaaS) is a near-sound-alike. | **Medium** |
| 6 | **Guildry** | A guild of craftsmen. | taken | **free** | taken | `getguildry.com`, `.ai` taken | org exists (2 repos) | 0 | A 0★ concept repo ("project intelligence platform for services firms"). The real risk is **Guild.ai**, an AI-agent control plane with a $44M seed and Series A (GV, Khosla) in the same category. | 0 results | **Low–Medium** (confusion with Guild.ai) |
| 7 | **Thingstead** | The Norse *thing*-stead, the assembly place where the free gathered to decide. | taken | **free** | taken | `getthingstead.com` free, `.ai` taken | org exists (0 repos) | 0 | 0★ repos, two of them AI-governance or agent experiments. Used as a .NET namespace. | 0 results | **Low** (but obscure and odd-sounding) |
| 8 | **Tinkerage** | A place of tinkering. | taken | **free** | **free** | n/a | **free** | 0 | Nothing exact. The "Tinker" prefix is busy, including Thinking Machines Lab's *Tinker*, a prominent AI fine-tuning API. | not searched | **Low–Medium** |
| 9 | **Bothy** | A Scottish open shelter for travellers: one hut per bot. | taken | taken | taken | n/a | taken | 0 | **`bspeelm/bothy`**: a "terminal workspace" with an AI agent pane and podman confinement (4★, tagged `ai-coding`). Near-name *Bothread* coordinates Claude Code and Codex. | Only GIN BOTHY (class 33) live | **Medium** |
| 10 | **Orrery** | A clockwork model of planets moving together. | taken | taken | taken | n/a | taken | 9 (astronomy apps) | `divyamtalwar/orrery`, an MCP server that orchestrates multiple AI coding agents. `ItIsMixian/Orrery`, an agent skill. Heavy generic use. | **ORRERY** (TrueDataLab, filed Apr 2026, class 42, AI-based SaaS) | **Medium** |
| 11 | **Keelson** | The keelson: the ship's internal backbone timber. | taken | taken | taken | n/a | taken | 2 | **`akapril/keelson`**, a "local-first AI workbench" for Claude and Codex sessions (Tauri, Rust): almost this product. Also Keelson for spec-driven Claude Code, `keelson-ai`, and the "Adopt Keelson" agent skill. | KEELSON in classes 36 and 19. Dead class 42. | **Medium–High** |
| 12 | **Rookery** | A colony of rooks. | taken | taken | taken | n/a | org exists | 4 | **`rookery-ai/rookery`**: self-hosted, runs AI agents "around the clock" on your machine, defaults to the `claude` CLI (v0.1.1, Aug 2026). | **ROOKERY registered, class 42 SaaS** (American Cyborg) | **High** |

### Also checked and eliminated (direct competitors with the same name)

- **Bosun:**
  - `virtengine/bosun`, an open-source "supervisor and orchestrator for AI coding agents" (Copilot, Codex, Claude; full PR lifecycle; npm `bosun`, about 260★);
  - **Bosun.ai**, a funded Dutch company whose AI agents open merge-ready PRs (`bosun-ai/kwaak`, "Burn through tech debt with AI agents!");
  - the App Store's "Bosun App" (Developer Tools);
  - Stack Exchange's `bosun-monitor/bosun` (3.4k★).

  **High.**
- **Muster:**
  - **launched on Product Hunt on 2026-10-14** as "the native cockpit for AI coding agents": git worktrees, PRs, macOS/Linux/Windows, $39, musterup.dev;
  - `amterp/muster`, a macOS terminal for several agents;
  - Giant Swarm's `muster` MCP gateway, which uses `~/.config/muster`.

  **High.**
- **Covey:**
  - `benjaminLedel/covey` ("runs AI agents like employees");
  - COVEY **registered in class 42 for AI SaaS** (Covey Inc), with about 91 COVEY marks in total.

  **High.**
- **Crewhaus:** an active "CrewHaus" AI-agent meta-harness (`crewhaus/factory`, on winget), close to CrewAI. **High.**
- **Pilothouse:** an existing Docker dev-environment CLI, plus two pending PILOTHOUSE class 42 SaaS filings. **High.**
- **Myrmex:** myrmex.ai, a commercial multi-agent platform (an orchestrator directing specialist agents, plus an endpoint agent on Windows, Linux and macOS). **Medium–High.**
- **Coxswain:** an active `coxswain` developer file manager on crates.io, with a desktop app. **Medium.**
- **Lemnos:** no software conflict, but github.com/lemnos is a known developer (theme.sh, 959★) and LEMNOS is a live mark of a startup VC. **Low–Medium.**
- **Botwright:** clean (0 TM results, 3★ Discord-bot testing repo), but it sits next to *Botright* and Playwright. **Low.**
- **Petasos** (Hermes's winged hat): the App Store already lists "Petasos: Agent Cockpit" in Developer Tools. **High.**

### Package registries (finalists)

| name | crates.io | npm | PyPI | Homebrew |
|---|---|---|---|---|
| hiveyard | free | free | free | free |
| telchine | free | free | free | free |
| botswain | free | free | free | free |
| guildry, thingstead, tinkerage | free | free | free | free |
| wardroom | free | taken | taken | free |
| flotilla | taken | taken | taken | free |

## Top 5, ranked

| rank | name | rationale | daemon / CLI | home | macOS bundle ID if ever changed (the impact map recommends keeping `com.manuelrinaldi.thehermes`) | launchd label |
|---|---|---|---|---|---|---|
| 1 | **Hiveyard** | Self-explanatory: many busy workers, each in its own hive (VM), managed from one yard. No software or TM conflict found, and `.dev`, `.app`, the GitHub handle and every package name are free. | `hiveyardd` reads awkwardly with the double d. Consider a single `hiveyard` CLI with `hiveyard daemon` (service label `…hiveyard.daemon`), or the short alias `hyd`. | `~/.hiveyard` | `com.manuelrinaldi.hiveyard` | `com.manuelrinaldi.hiveyardd` |
| 2 | **Telchine** | Mythic and distinctive. It keeps the Greek lineage of "Hermes" with a better fit (master craftsmen). It's the cleanest name on every check except `telchine.ai`. The cost is pronunciation: *TEL-kine*, from *Telchines*. | `telchined` | `~/.telchine` | `com.manuelrinaldi.telchine` | `com.manuelrinaldi.telchined` |
| 3 | **Botswain** | A witty, instantly "crew" name: the bosun runs the deck crew while you captain. Fully clean (0 TM, `.dev`, `.app`, `getbotswain.com`, `.ai` and the packages all free). Risks: it can be misheard as "boatswain" or misread as "Botswana", and a coined pun is a weaker mark. | `botswaind` | `~/.botswain` | `com.manuelrinaldi.botswain` | `com.manuelrinaldi.botswaind` |
| 4 | **Wardroom** | The best fit for meetings, drafts and owner approvals ("where the officers decide"). Clean trademark picture, and the GitHub handle is free. But all domains and the npm/PyPI names are taken, and "war room" is crowded in agent tooling. | `wardroomd` | `~/.wardroom` | `com.manuelrinaldi.wardroom` | `com.manuelrinaldi.wardroomd` |
| 5 | **Flotilla** | The strongest metaphor for "each bot gets its own computer" and linked machines: a fleet of small boats. Most memorable of the five. No live software trademark found, but every domain and package name is taken, and several small agent and infra projects use it. | `flotillad` | `~/.flotilla` | `com.manuelrinaldi.flotilla` | `com.manuelrinaldi.flotillad` |

Runners-up: **Guildry**, a clean exact name held back by its closeness to the
funded Guild.ai, and **Thingstead**, clean but obscure.

### If one of these is picked, before committing

1. Run a real USPTO and EUIPO knockout (classes 9 and 42) and check the UK and
   WIPO registers. None of the trademark findings above come from an official
   registry.
2. Register `<name>.dev` and `<name>.app` together, plus `get<name>.com`.
   Optionally take the GitHub org and handle, the crates/npm names and an iOS
   App Store name reservation. That is the owner's action; nothing was
   registered here.
3. Re-search the App Store and Product Hunt on the day of the decision. Muster
   launched as a direct competitor **today**, which shows how fast this space
   is moving.

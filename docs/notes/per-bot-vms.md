# A computer for every bot: per-bot VMs with dockur and bollard

Research and design only. No product code was changed and nothing was pushed.
Written 2026-10-14 against `gravity-wt-k9` (branch `k9-acceptance`, which is current
with `release-0.19.0-kanban-rework`). File references are relative to that checkout.

## 0. Summary and recommendation

- **What the owner wants.** "Each bot gets its own computer, similar to grob bot."
  That is most likely **xAI's Grok Bot**: a persistent Linux cloud computer with a
  browser, files and a terminal, and a *Watching* / *Take control* screen on desktop
  and phone. Grok Bot actually shares **one** computer per account across all bots.
  Gravity can do better, with **one computer per bot** on the owner's own machines.
- **Recommendation.** Treat each bot's computer as a **linked computer**: run a
  hermesd in "computer mode" *inside* the guest, paired with the host as a new
  **`computer` peer class** with **asymmetric trust**. The guest trusts its host; the
  host trusts nothing owner-like from the guest.
  - Inside the guest, everything that breaks across a VM boundary keeps working
    unchanged: process-lineage bus identity, inbox sockets, the PTY, transcripts,
    Codex, the agent browser and the MCP allowlist.
  - The host reuses the existing peer relays for chat, terminal, browser, tasks,
    artifacts and boards.
  - The alternative, a per-VM bearer token plus a thin in-guest agent, would mean
    rebuilding about ten session subsystems as remote calls.
- **First target: Ubuntu, as a desktop container rather than a VM.** It needs no KVM
  and no licence, and it runs **today on this M4 Mac** (colima has no `/dev/kvm`;
  measured), on the Windows PC and on Linux. Full VMs come after it:
  - `qemux/qemu` Ubuntu and `dockurr/windows`, wherever `/dev/kvm` exists: Linux,
    Windows 11 + WSL2, M3+ Macs with colima `--nested-virtualization`, which gives
    arm64 guests only;
  - macOS through **tart or lima on Apple silicon**, at most two per Mac by Apple's
    licence. `dockur/macos` is x86-only and EULA-restricted to Apple hardware.
- **The Browser tab becomes a Computer tab.** It shows the guest's desktop, streamed
  by hermesd from the guest's VNC (QEMU's own VNC for VMs, x11vnc for containers) and
  re-encoded as the JPEG frames the browser screencast already uses. That works on
  the phone and across linked computers without new auth. *Take control* maps
  today's `browser_input` events to VNC input. A raw VNC tunnel with noVNC follows
  for smoother control. The bot's Chrome runs headed on that desktop, and the
  browser activity log stays.
- **bollard 0.21.1** fits hermesd's Tokio and hyper 1 stack, supports Unix sockets
  and Windows named pipes, and was exercised here end to end: pull with progress,
  create with limits, ports, labels and healthcheck, exec, logs, remove. It does
  **not** read Docker contexts, so hermesd must resolve the colima or Docker Desktop
  socket itself.
- **Security caveats.**
  - Guests can reach the host's `127.0.0.1` services (measured under colima).
  - Anything that can reach the Docker socket controls every computer.
  - VNC and noVNC ports must stay on loopback and be reachable only by the daemon.
- **MVP.** One Ubuntu desktop container per opted-in bot. A guest hermesd paired as a
  computer peer. Computer tab watching and Take control on the desktop app,
  watch-only on the phone. About 3-4 weeks of work. §9 lists the files and modules
  it touches.

---

## 1. Assumptions (the owner was away; these stand until corrected)

1. **The goal is a computer the bot owns, not only isolation.** Each bot gets a
   persistent desktop with a browser, files and apps. The owner can watch it and take
   it over from a **Computer** tab that replaces today's Browser tab. This follows the
   owner's "similar to grob bot" clarification; see §2 for which product that probably is.
2. **Guests are Ubuntu, Windows and macOS, in that order.** Ubuntu comes first because
   it runs on the most hosts, including Apple-silicon Macs, and has no licensing
   questions. Windows comes second, for Windows build and install work. macOS comes
   last: Apple's licence and the hardware limit how it can run (§9).
3. **A computer is opt-in per bot.** Bots without one keep running exactly as today.
   Workers (temporary bots) don't get one in the first phases.
4. **One owner, one tailnet.** The trust model in `docs/architecture.md` stays the
   same: Gravity is single-user. A VM adds a boundary *around a bot*. It is not a
   multi-tenant sandbox.
5. **Docker is the substrate wherever it exists.** It is already on the owner's Macs
   (colima) and the Windows PC (Docker Desktop for the visual tests). hermesd drives
   Docker through bollard. Where Docker can't give a VM (macOS guests on Apple
   silicon), the same "computer" contract is met by another provider (§7).
6. **The owner's Claude or Codex subscription is used inside the guests.** Each guest
   is signed in with a token the daemon injects. Bots on the host can already read
   these credentials today (they run as the owner's user), so putting them in a guest
   loses nothing.
7. **The owner's machines are assumed to be:**
   - this Mac (Apple **M4**, 10 cores, 24 GB RAM, macOS 27, colima 4 CPU / 6 GB with
     `vmType: vz`; measured);
   - other Apple-silicon Macs (chip unknown, assumed M1 to M4);
   - an **iMac**, assumed to be an *Intel* iMac (if it is an Apple-silicon iMac,
     treat it like the other Macs);
   - a **Windows PC**, assumed to be Windows 11 x64 with VT-x/AMD-V enabled and
     Docker Desktop on WSL2.

---

## 2. "Similar to grob bot": what the owner probably means, and how Gravity compares

**Most likely: xAI's "Grok Bot".** It was announced on 2026-08-11 as an early beta.
Its pitch is that each bot has "a computer of its own", runs 24/7, signs in to your
tools, and comes back to you only for approvals. Bots coordinate through a "chief of
staff" bot and message each other in threads. No product named "grob bot", "grobbot"
or "grob.ai" turned up; searches for them land on Grok Bot. This is an inference, so
it is open question Q1.

- Launch and pitch: https://x.ai/news/introducing-grok-bot,
  https://interestingengineering.com/ai-robotics/xai-grok-bot-computer-agent
- Docs: https://docs.x.ai/grok-bot/overview, https://docs.x.ai/grok-bot/faq

What a Grok Bot gets, per its docs:

- **A persistent cloud computer**, described as a "managed Linux virtual machine",
  with a browser, a filesystem and a terminal. Work continues with the laptop closed.
  The computers run in Cursor's cloud.
- **The computer is per account, not per bot.** "The computer belongs to your
  account, not to an individual Bot." All bots share its files, browser sessions and
  logins. "Each Bot gets its own screen on the shared computer", and the FAQ warns
  "Do not use separate Bots as a security boundary"
  (https://docs.x.ai/grok-bot/faq; analysis:
  https://www.digitalapplied.com/blog/grok-bot-ai-teammates-launch-cloud-computer-2026).
- **Watch, then take over.** The full-screen computer view opens in *Watching* mode
  until you press *Take control*. Takeover is how passwords, 2FA, CAPTCHAs and
  payments are handled: you do that step, then tell the bot to continue. Mobile can
  watch and take over too.
  - The changelog (https://x.ai/changelog/bot) returned 403, so this comes from search
    excerpts.
  - https://flaviocopes.com/grok-bot/
- **Skill recording:** show a bot a workflow once and it becomes a reusable routine.
- **Clients:** macOS, Windows and iPhone. Cloud-only and proprietary. Pricing reports
  conflict (bundled with SuperGrok/Cursor tiers, or $200/month).

Other products of the same kind, for reference:

| Product | Computer | Screen | Take over | Self-host |
|---|---|---|---|---|
| **Bytebot** (archived 2026-03-07, Apache-2.0) | Ubuntu 22.04 + XFCE container: Firefox, Thunderbird, VS Code, 1Password | Xvfb → x11vnc → noVNC/websockify on :9990 | Button, plus auto-takeover on "needs help"; human actions logged | Yes. https://github.com/bytebot-ai/bytebot, https://docs.bytebot.ai/guides/takeover-mode |
| **Cua / Cua Spaces** (MIT / FSL) | macOS and Linux VMs on Apple Virtualization.framework (Lume); `cua-xfce` container | Spaces: own low-latency stream; container: TigerVNC + noVNC 6901 | Watch and take over | Yes. https://github.com/trycua/cua |
| **E2B Desktop** (Apache-2.0 SDK, cloud) | Ubuntu 22.04 XFCE sandbox | x11vnc + noVNC 6080, `getUrl({viewOnly})` | View-only vs control URLs | Cloud. https://github.com/e2b-dev/desktop |
| **Anthropic computer-use demo** | ubuntu:22.04, Xvfb, mutter, Firefox, LibreOffice | x11vnc 5900, noVNC 6080 | Through noVNC | Reference. https://github.com/anthropics/claude-quickstarts/tree/main/computer-use-demo |
| ChatGPT agent, Manus, Orgo, Scrapybara | Cloud VMs (Ubuntu; Orgo Windows beta) | Hosted viewers | Takeover in ChatGPT agent | No |

**Gravity with dockur and bollard, against Grok Bot:**

| | Grok Bot | Gravity, as proposed |
|---|---|---|
| Whose computer | One per account, shared by every bot | **One per bot**: its own files, logins and installs. A real security boundary between bots (with the caveats in §7.6) |
| Where | xAI/Cursor cloud | The owner's own machines (Docker on the Macs, the Windows PC, a Linux box), reached over the existing tailnet links |
| OS | Linux | **Ubuntu** first; **Windows** (dockur); **macOS** on Apple silicon (tart/lima); macOS via dockur only on an Intel Mac running Linux |
| Persistent | Yes | Yes (a volume or disk per bot), plus **reset to a clean image** and snapshots |
| Browser | In the computer | In the computer (the bot's Playwright Chrome runs *headed* on its desktop), and today's per-tab screencast and activity log are kept |
| Watching / Take control | Yes, desktop and phone | **Computer tab** with *Watching* / *Take control* (§8), on desktop, phone and across linked computers |
| Bot-to-bot | Threads, chief-of-staff | Already richer: durable bus, tasks with deadlines and hop limits, boards, decisions, owner actions, routines |
| Skill recording | Yes | Not planned here. Routines and FACTS.md are the closest. |
| Cost | Subscription | The owner's hardware (RAM is the limit) plus the owner's Claude/Codex plan; Windows licences for Windows guests |

The experience to copy is **"open the bot and see its computer, live; press Take
control, do the step, hand it back"**. Gravity already has that shape for the browser
(`docs/bot-browser.md`, "Take control" / "Give back control"). The design below widens
it from a tab to a whole desktop.

---

## 3. Host matrix: which machine can host which guest

The rules that decide it:

- **dockur and qemus need `/dev/kvm` in the container.** Their READMEs say: "Docker
  Desktop on Linux, macOS, and Windows 10 does not currently provide KVM access to
  containers". Supported setups are Linux with KVM, and **Windows 11** Docker Desktop
  or Podman Desktop with nested virtualization.
- **Apple silicon nests only on M3+ with macOS 15+**, and only for Linux guests
  (`VZGenericPlatformConfiguration.isNestedVirtualizationSupported`: "available for
  Mac with the M3 chip, and later").
  - Colima exposes it as **`--nested-virtualization`** / `nestedVirtualization: true`
    (vz). The flag only exists on capable Macs.
  - Lima exposes it as `nestedVirtualization: true` (≥ 1.0).
  - Docker Desktop (Docker VMM) does **not** expose it.
  - OrbStack had promised it but not shipped it as of v2.2.4 (unverified).
- **On Apple silicon, KVM is arm64-on-arm64.** x86 Windows or macOS would run under
  TCG at "about 10 times slower". That rules out `dockurr/macos` (x86 OpenCore + AVX2)
  on any Apple-silicon Mac, nested or not. `dockurr/windows` on arm64 resolves to
  **Windows 11 ARM**.
- **Intel Macs give containers no KVM** under macOS (Docker Desktop, HyperKit and
  Virtualization.framework don't nest on Intel). Linux installed on the iMac gives a
  native `/dev/kvm`.
- **WSL2's `nestedVirtualization` defaults to true and is Windows 11 only.** Hyper-V
  nesting needs Intel VT-x+EPT, or AMD on Windows 11 / Server 2022 or later.
- **macOS guests by licence:** at most **2 extra macOS instances** per Apple computer
  that already runs macOS, enforced by Virtualization.framework (a third fails with
  VZErrorDomain code 6).

| Owner's machine | Ubuntu (P1 container) | Ubuntu (P2 full VM) | Windows (P3 dockur) | macOS (P4 tart/lima) | macOS (P5 dockur) |
|---|---|---|---|---|---|
| **This Mac: M4, macOS 27, colima vz** | **Yes, today.** arm64 container in colima, no KVM needed | **Yes, after** `colima stop; colima start --nested-virtualization` (an owner action; not done here). arm64 Ubuntu with `qemux/qemu-arm` | **Windows 11 ARM** only, with nested colima (`dockurr/windows` → windows-arm). Untested upstream on Apple silicon. Needs RAM: raise colima to ≥ 10-12 GB | **Yes**, via tart or lima `template:macos` (≤ 2 at a time). Not through Docker | **No** (x86 only) |
| **Other Apple-silicon Macs, M3/M4** | Yes | Same as above | Same as above (ARM) | Yes | No |
| **Apple-silicon Macs, M1/M2** | **Yes** | **No** KVM (no nesting). Only TCG, ~10× slower. Use P1 | **No** (not practical) | Yes (tart/lima) | No |
| **iMac (assumed Intel) on macOS** | Yes (Docker Desktop or colima, x86 container) | No | No | Only via VMware/Parallels; not recommended | No |
| **iMac (Intel) with Linux installed** | Yes | **Yes** (KVM) | **Yes**, x86 Windows | n/a | **Yes**: the *only* EULA-compatible dockur/macos host (Apple hardware), if its CPU has AVX2 (Haswell 2013+) |
| **Windows PC, Windows 11 + Docker Desktop (WSL2)** | Yes | **Yes**, nested in WSL2 | **Yes**, x86 Windows 11. Watch for the WSL2 install hang (#1156) | No | Technically yes on AVX2, but **not licence-compliant** (not Apple hardware) |
| Windows PC, Windows 10 | Yes | No | No | No | No |
| A Linux box or server with KVM | Yes | Yes | Yes | No | Only if Apple hardware |

**Practical placement for the owner:**

- **Ubuntu computers run anywhere**, today, as P1 containers. On M3+ Macs or the
  Windows PC they can be full VMs.
- **Windows computers run on the Windows PC** (x86, dockur). An M3+ Mac can host
  Windows 11 ARM for ARM-native work. Gravity's Windows release is x64 only, so x64
  install testing belongs on the PC.
- **macOS computers run on Apple-silicon Macs through tart or lima**, at most two per
  Mac.
- The iMac becomes useful for dockur only if it runs Linux. It would then be the one
  machine that can host x86 macOS legally, plus x86 Windows and Ubuntu VMs.

Sources:

- https://raw.githubusercontent.com/dockur/windows/master/readme.md,
  https://raw.githubusercontent.com/dockur/macos/master/readme.md
- https://developer.apple.com/tutorials/data/documentation/virtualization/vzgenericplatformconfiguration/isnestedvirtualizationsupported.json
- https://raw.githubusercontent.com/abiosoft/colima/main/cmd/start.go,
  https://raw.githubusercontent.com/abiosoft/colima/main/embedded/defaults/colima.yaml,
  https://github.com/abiosoft/colima/releases (latest v0.10.3)
- https://lima-vm.io/docs/config/vmtype/vz/, https://lima-vm.io/docs/usage/guests/macos/
- https://docs.docker.com/desktop/features/vmm/
- https://github.com/orbstack/orbstack/issues/1504
- https://learn.microsoft.com/en-us/windows/wsl/wsl-config,
  https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/enable-nested-virtualization
- https://eclecticlight.co/2022/08/04/virtualisation-on-apple-silicon-macs-8-how-apple-limits-vms/
- Measured on this Mac: §6.1.

**Lighter alternatives, compared:**

| | Good for | Limits | Licence |
|---|---|---|---|
| **Plain Linux container per bot** (P1, or headless) | Linux-only work. Cheapest. Runs on every host | Shared kernel; no Windows/macOS | Free |
| **tart** (now `github.com/openai/tart`, 2.41.0) | macOS (and Linux) VMs on Apple silicon. OCI images (`tart clone ghcr.io/cirruslabs/macos-tahoe-base`), `--vnc`, `--dir name:path:ro`, `tart exec` through a guest agent, `--nested` for Linux on M3+ | Apple silicon only; 2 macOS VMs per host | **FSL-1.1-ALv2** in the repo now (internal use OK, each release becomes Apache-2.0 after 2 years). The older tart.run "free up to 100 cores" page may be stale. https://raw.githubusercontent.com/openai/tart/main/LICENSE, https://tart.run/quick-start/ |
| **lima** (vz) | Linux VMs; **macOS guests since Lima 2.1** (`limactl start template:macos`); `video.display: vnc` | macOS guest is experimental (no headless display, no automatic port forwarding) | Apache-2.0 |
| **Apple `container`** (1.5.0) | One lightweight VM per Linux container on macOS 26 | Linux only; young | Apache-2.0. https://github.com/apple/container |
| **Windows Sandbox** | Throwaway Windows; `.wsb` mapped folders and `LogonCommand`; `wsb` CLI on 24H2 | **One instance at a time**, nothing persists, Pro/Enterprise only | Included with Windows |
| **Hyper-V** | Persistent Windows VMs on the PC with checkpoints (`Checkpoint-VM`, `Restore-VMCheckpoint`) | PowerShell or WMI driven, not Docker. A second provider to write | Included (Pro+) |
| gVisor / Kata / Firecracker | Stronger Linux sandboxes | Linux only, need KVM (except gVisor systrap) | Open source |

---

## 4. dockur facts, with sources

All projects are by the same author (kroese) and MIT-licensed. Every VM image is a
thin layer over **`qemux/qemu`** (repo `qemus/qemu`). The dockur org
(https://github.com/orgs/dockur/repositories) publishes:

- desktop OS images: `windows` (~54k stars), `macos` (~22k), `windows-arm`
  (~2.3k), `chromeos`;
- appliance images: casa, umbrel, zima, the Proxmox family, virtual-dsm;
- **no `dockur/linux`**. The Windows and macOS READMEs send Linux users to
  `qemus/qemu`.

| | `dockurr/windows` | `dockurr/windows-arm` | `dockurr/macos` | `qemux/qemu` / `qemux/qemu-arm` |
|---|---|---|---|---|
| Latest (2026-10-14) | **v6.06**, 2026-10-05 | v6.06, 2026-10-05 | **v3.12**, 2026-08-27 | **v7.50**, 2026-09-08 (QEMU 11.1; adds the QEMU Guest Agent) |
| Container image (compressed) | amd64 239 MB / arm64 165 MB (**arm64 resolves to windows-arm** via `FROM build-${TARGETARCH}`) | arm64 165 MB | **amd64 only**, 161 MB | 178 / 142 MB; arm 144 / 141 MB |
| Needs | `--device /dev/kvm --device /dev/net/tun --cap-add NET_ADMIN`; `privileged` **not** needed | Same | Same + **AVX2 CPU** | Same |
| Defaults | 2 CPU, 4G RAM, 64G disk (`CPU_CORES`, `RAM_SIZE`, `DISK_SIZE`); minimum 2 GB RAM, 32 GB disk | 2 CPU, 4G, 64G | 2 CPU, 4G, 64G | 2 CPU, **2G**, 64G |
| Ports | **8006** noVNC web viewer, **3389** RDP tcp/udp, 5900 VNC (`DISPLAY=vnc`, `VNC_PORT`) | Same | 8006, 5900; `EXPOSE 22` | 8006 |
| Guest download | ISO from Microsoft (`MIDO=Y`): **11 Pro 7.9 GB**, 11 LTSC 4.7 GB, 11 Enterprise 6.6 GB, Server 2025 7.6 GB, tiny11 5.3 GB, core11 3.0 GB; `REMOVE=Y` deletes the ISO after install | 11: 7.5 GB, 11 LTSC 4.7 GB, tiny11 5.1 GB | Apple recovery image, then the installer from Apple. No size given (under ~1 GB recovery, from general knowledge) | Distro ISO (`BOOT=ubuntu` desktop, `ubuntus` server, `xubuntu`, `kubuntu`, …, a URL, or `/boot.iso`/`.img`/`.qcow2`) |
| Unattended install | **Yes**, fully automatic. Answer files are built in. `USERNAME`/`PASSWORD` (default `Docker`/`admin`), `LANGUAGE`, `REGION`, `KEYBOARD`, `KEY`, `EDITION`, `HOST`, `DOMAIN`. **`/oem` is copied to `C:\OEM` and `install.bat` runs at the end of setup**; `COMMAND` runs one command | Same | **No**. Manual: erase the VirtIO disk as APFS, Reinstall macOS, create a local account | No (an interactive ISO install). A cloud image (`/boot.qcow2`) + cloud-init would be unattended (inference) |
| First boot | "A few minutes" after the download; not measured. Known hang on WSL2 Docker Desktop at "Copying files 0%" (#1156); workarounds `DISK_FMT=qcow2`, smaller `DISK_SIZE`, `ADAPTER=e1000e` | n/a | Long and manual | Install from ISO |
| Shared folder | `-v ./x:/shared` → "Shared" on the desktop and **`Z:`**; Samba share `\\host.lan\Data` (`/data` still works; `/shared2`…) | Same | `/shared` → `sudo -S mount_9p shared` in the guest; no virtiofs | `/shared` over **9p** (`mount -t 9p -o trans=virtio shared /mnt/x`) |
| Guest → host network | NAT. The guest gets a 172.30+.x subnet. **`host.lan` = the container gateway**; `system.lan` is DNAT'd to the Docker host side. A catch-all DNAT forwards **every TCP port** of the container to the guest except the container's own, so publishing a port reaches the guest. Fallback is user-mode (passt/slirp) with `USER_PORTS`. `DHCP=Y` uses macvtap on a macvlan network | Same | Same | Same |
| Snapshots / reset | **No snapshot feature.** `DISK_FMT=raw` (default) or `qcow2`; disks sparse unless `ALLOCATE=Y`; grow only. Reset means deleting `/storage` (inference). `CLEAR=Y` resets NVRAM. `QMP`/`MONITOR`/`QGA` endpoints exist | Same | Same | Same |
| Without KVM | Exits with code 88. `KVM=N` → TCG, "about 10 times slower" | No accelerator in TCG on non-amd64 (very slow) | Not practical | `KVM=N` → TCG |
| Hosts per README | Linux with KVM; Windows 11 Docker Desktop/Podman Desktop with nested virtualization. "Docker Desktop on Linux, macOS, and Windows 10 does not currently provide KVM access to containers" | Same (aimed at ARM64 Linux, "like the Raspberry Pi 5") | Same | Same |
| Licence notes | "does not distribute Windows itself"; the built-in keys are "generic installation keys … for trial purposes", not activation. Bring a `KEY` | Same | "Only run this container on Apple hardware, any other use is not permitted by their EULA" | Distro licences |

**Display and viewer.** `PROTECT=Y` password-protects the web UI. The README
recommends RDP for daily use: the web viewer is mainly for installation and lacks
features like the clipboard (qemus v7.50 is adding noVNC clipboard). Other options are
`WIDTH`/`HEIGHT` (1280×720 for Windows, 1920×1080 for macOS), `GPU` (v6.06 "Helios"
acceleration), `AUDIO`.

Sources:

- https://github.com/dockur/windows,
  https://raw.githubusercontent.com/dockur/windows/master/readme.md,
  https://raw.githubusercontent.com/dockur/windows/master/docs/environment.md,
  https://raw.githubusercontent.com/dockur/windows/master/Dockerfile,
  https://raw.githubusercontent.com/dockur/windows/master/src/samba.sh,
  https://github.com/dockur/windows/releases,
  https://github.com/dockur/windows/issues/1156
- https://github.com/dockur/windows-arm
- https://github.com/dockur/macos,
  https://raw.githubusercontent.com/dockur/macos/master/docs/environment.md
- https://github.com/qemus/qemu,
  https://raw.githubusercontent.com/qemus/qemu/master/src/network.sh,
  https://raw.githubusercontent.com/qemus/qemu/master/src/disk.sh,
  https://raw.githubusercontent.com/qemus/qemu/master/src/proc.sh,
  https://github.com/qemus/qemu-arm
- Docker Hub tags: https://hub.docker.com/v2/repositories/dockurr/windows/tags
- Compressed sizes were also checked here with `docker manifest inspect` (§6.1).

The README compose for Windows (the macOS one adds `5900`, and uses `VERSION: "15"`):

```yaml
services:
  windows:
    image: dockurr/windows
    environment: { VERSION: "11" }
    devices: [/dev/kvm, /dev/net/tun]
    cap_add: [NET_ADMIN]
    ports: ["8006:8006", "3389:3389/tcp", "3389:3389/udp"]
    volumes: ["./windows:/storage"]
    restart: always
    stop_grace_period: 2m
```

**Ubuntu without a VM.** The P1 container options:

- **Our own image**, the recommended one. It is the stack Bytebot
  (https://raw.githubusercontent.com/bytebot-ai/bytebot/main/packages/bytebotd/Dockerfile),
  the Anthropic computer-use demo
  (https://raw.githubusercontent.com/anthropics/claude-quickstarts/main/computer-use-demo/Dockerfile)
  and E2B Desktop use: Ubuntu, Xvfb, XFCE or mutter, x11vnc, Chrome or Firefox.
  It needs no arch-specific steps beyond Chrome (Chromium or Playwright's Chromium on
  arm64).
- **`linuxserver/webtop`** (amd64 and arm64, Ubuntu XFCE/KDE/MATE tags rebased on
  Ubuntu 26.04 on 2026-04-07). It has **used Selkies since 2025-06-17** (ports
  3000/3001, `--shm-size=1gb`), has no auth by default and has passwordless sudo
  (https://docs.linuxserver.io/images/docker-webtop/).
- **Kasm images** (`kasmweb/ubuntu-noble-desktop`, KasmVNC on 6901, `VNC_PW`): they
  use KasmVNC, which isn't RFB-compatible (https://github.com/kasmtech/workspaces-images).
- **`accetto/ubuntu-vnc-xfce-g3`** (TigerVNC + noVNC, MIT).

---

## 5. bollard: API sketch for hermesd

**Maturity.**

- Latest is **0.21.1** (2026-08-16), after 0.21.0 (2026-05-04) and 0.20.x (Jan to
  Mar 2026). It is actively maintained by fussybeaver and Apache-2.0.
- 0.20.0 moved to Docker API 1.52 and **removed the deprecated option structs**.
  0.21.0 pegs API 1.53 and adds Podman support. 0.20.1 added `connect_with_host`.
- No MSRV is declared; CI built with Rust 1.95.
- Dependencies: tokio 1.47+, hyper 1.3+ / hyper-util, `hyperlocal` (Unix),
  `hyper-named-pipe` (Windows). Optional: rustls, ssh, buildkit, websocket.
- **Docker contexts are not read by 0.21.1.** Unreleased master has
  `context::resolve_host`. Resolve contexts yourself until it ships.
- Connection helpers: `connect_with_unix_defaults` honours `DOCKER_HOST` only when
  it is `unix://…`; `connect_with_named_pipe_defaults` ignores it;
  `connect_with_defaults` dispatches on its scheme.

Sources:

- https://crates.io/api/v1/crates/bollard, https://github.com/fussybeaver/bollard/releases
- https://docs.rs/crate/bollard/0.21.1/source/Cargo.toml.orig
- https://docs.rs/crate/bollard/0.21.1/source/src/docker.rs
- https://raw.githubusercontent.com/fussybeaver/bollard/master/src/docker.rs
- https://docs.rs/bollard/latest/bollard/models/struct.HostConfig.html
- https://raw.githubusercontent.com/fussybeaver/bollard/v0.21.1/examples/exec.rs
- Endpoints:
  - Docker Desktop for Mac uses `desktop-linux` → `~/.docker/run/docker.sock`
    (https://docs.docker.com/desktop/setup/install/mac-permission-requirements/);
  - colima uses `~/.colima/default/docker.sock`
    (https://github.com/abiosoft/colima/blob/main/docs/FAQ.md);
  - rootless uses `$XDG_RUNTIME_DIR/docker.sock`
    (https://docs.docker.com/engine/security/rootless/tips/);
  - context metadata is in `~/.docker/contexts/meta/<sha256(name)>/meta.json`
    (https://docs.docker.com/engine/manage-resources/contexts/).

The snippets below compile against **bollard 0.21.1** (the create-container sketch was compile-checked as `bollard-probe/src/bin/sketch.rs`; the rest ran). They are taken from the
working probe in the scratchpad (`bollard-probe/src/main.rs`), which ran against
colima. bollard 0.19+ uses generated option builders (`bollard::query_parameters::*`)
and `bollard::models::ContainerCreateBody` in place of the older
`bollard::container::Config`.

```toml
# crates/hermesd/Cargo.toml (proposed)
bollard = { version = "0.21", default-features = false, features = ["pipe"] } # unix socket + Windows named pipe
```

**Finding the engine.** bollard does **not** read Docker CLI contexts. hermesd must
resolve the endpoint itself, in this order:

1. `[computers] docker_host` in `hermesd.toml`;
2. `DOCKER_HOST`;
3. the CLI's `currentContext` in `~/.docker/config.json`, then
   `~/.docker/contexts/meta/<sha256(name)>/meta.json` → `Endpoints.docker.Host`;
4. well-known sockets:
   - colima: `~/.colima/default/docker.sock`;
   - Docker Desktop: `~/.docker/run/docker.sock`;
   - `/var/run/docker.sock`;
   - rootless: `$XDG_RUNTIME_DIR/docker.sock`;
   - Windows: `//./pipe/docker_engine`, or `dockerDesktopLinuxEngine`.

This matters because the daemon runs under launchd or Task Scheduler with a minimal
environment, where `DOCKER_HOST` is usually unset.

```rust
use bollard::{Docker, API_DEFAULT_VERSION};
use sha2::{Digest, Sha256};

fn docker_from_cli_context(home: &Path) -> Option<String> {
    let cfg: serde_json::Value =
        serde_json::from_slice(&std::fs::read(home.join(".docker/config.json")).ok()?).ok()?;
    let ctx = cfg.get("currentContext")?.as_str()?;
    let meta = home.join(".docker/contexts/meta")
        .join(hex::encode(Sha256::digest(ctx.as_bytes())))
        .join("meta.json");
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(meta).ok()?).ok()?;
    v.pointer("/Endpoints/docker/Host")?.as_str().map(str::to_string)
}

// unix:///… or npipe:////./pipe/…; 120 s request timeout.
let docker = Docker::connect_with_socket(&host, 120, API_DEFAULT_VERSION)?;
docker.ping().await?;
let info = docker.info().await?;            // NCPU, MemTotal, ID (engine id), OperatingSystem
```

**Pull with progress** (streams `CreateImageInfo`; map it to a `computer_progress` push):

```rust
use bollard::query_parameters::CreateImageOptionsBuilder;
use futures_util::StreamExt;

let mut pull = docker.create_image(
    Some(CreateImageOptionsBuilder::default().from_image("dockurr/windows").tag("6.06").build()),
    None, None);
while let Some(ev) = pull.next().await {
    let ev = ev?; // ev.status: "Downloading", ev.progress_detail: {current,total}, ev.id: layer
    progress.send(ev)?;
}
```

**Create a dockur Windows computer** with a KVM device, TUN, NET_ADMIN, loopback-only
ports, limits, labels, a storage volume, the OEM folder and a healthcheck:

```rust
use bollard::models::{ContainerCreateBody, DeviceMapping, HealthConfig, HostConfig,
                      Mount, MountType, PortBinding, RestartPolicy, RestartPolicyNameEnum};
use bollard::query_parameters::CreateContainerOptionsBuilder;

let dev = |p: &str| DeviceMapping { path_on_host: Some(p.into()),
    path_in_container: Some(p.into()), cgroup_permissions: Some("rwm".into()) };
let loopback = |port: &str| (format!("{port}/tcp"),
    Some(vec![PortBinding { host_ip: Some("127.0.0.1".into()), host_port: Some("0".into()) }]));

let body = ContainerCreateBody {
    image: Some("dockurr/windows:6.06".into()),
    env: Some(vec![
        "VERSION=11".into(), "RAM_SIZE=6G".into(), "CPU_CORES=4".into(), "DISK_SIZE=64G".into(),
        "DISK_FMT=qcow2".into(),                       // enables offline snapshots
        format!("USERNAME={user}"), format!("PASSWORD={generated}"),
        "DISPLAY=vnc".into(),                          // QEMU VNC on 5900 for the daemon's bridge
    ]),
    labels: Some([("dev.gravity.bot".into(), bot_id.clone()),
                  ("dev.gravity.home".into(), daemon_id.clone()),
                  ("dev.gravity.provider".into(), "dockur-windows".into())].into()),
    exposed_ports: Some(vec!["8006/tcp".into(), "5900/tcp".into(), "49777/tcp".into()]),
    healthcheck: Some(HealthConfig {
        test: Some(vec!["CMD-SHELL".into(), "nc -z 127.0.0.1 5900".into()]),
        interval: Some(10_000_000_000), timeout: Some(3_000_000_000),
        retries: Some(3), start_period: Some(60_000_000_000), ..Default::default() }),
    host_config: Some(HostConfig {
        devices: Some(vec![dev("/dev/kvm"), dev("/dev/net/tun")]),
        cap_add: Some(vec!["NET_ADMIN".into()]),
        port_bindings: Some([loopback("8006"), loopback("5900"), loopback("49777")].into()),
        memory: Some(7 * 1024 * 1024 * 1024),          // RAM_SIZE + headroom
        nano_cpus: Some(4_000_000_000),
        mounts: Some(vec![
            Mount { typ: Some(MountType::VOLUME), source: Some(format!("gravity-{bot_id}-storage")),
                    target: Some("/storage".into()), ..Default::default() },
            Mount { typ: Some(MountType::BIND), source: Some(oem_dir_on_engine_host),
                    target: Some("/oem".into()), read_only: Some(true), ..Default::default() },
        ]),
        restart_policy: Some(RestartPolicy { name: Some(RestartPolicyNameEnum::UNLESS_STOPPED), ..Default::default() }),
        ..Default::default()
    }),
    stop_timeout: Some(120),                           // dockur's stop_grace_period: 2m
    ..Default::default()
};
docker.create_container(Some(CreateContainerOptionsBuilder::default()
    .name(&format!("gravity-{bot_dir}")).build()), body).await?;
docker.start_container(&name, None).await?;
```

**Find the published ports, wait for health, and stream logs and events:**

```rust
let c = docker.inspect_container(&name, None).await?;
let port = c.network_settings.and_then(|n| n.ports)
    .and_then(|p| p.get("5900/tcp").cloned().flatten())
    .and_then(|b| b.first().and_then(|b| b.host_port.clone()));          // e.g. "32768"
let healthy = c.state.and_then(|s| s.health).and_then(|h| h.status);    // HealthStatusEnum::HEALTHY

use bollard::query_parameters::{LogsOptionsBuilder, EventsOptionsBuilder};
let mut logs = docker.logs(&name, Some(LogsOptionsBuilder::default()
    .follow(true).stdout(true).stderr(true).tail("200").build()));     // dockur prints install progress
let mut events = docker.events(Some(EventsOptionsBuilder::default()
    .filters(&HashMap::from([("label", vec!["dev.gravity.home=<id>"])])).build())); // die / oom / health_status
```

**Exec** (P1 containers: provisioning steps, or a `docker exec` terminal for the
owner; for dockur VMs it reaches only the QEMU container, so use it for `qemu-img`
or QMP):

```rust
use bollard::exec::{CreateExecOptions, StartExecResults};
let exec = docker.create_exec(&name, CreateExecOptions {
    cmd: Some(vec!["sh", "-c", "hermesd --version"]),
    attach_stdout: Some(true), attach_stderr: Some(true), tty: Some(false),
    ..Default::default() }).await?;
if let StartExecResults::Attached { mut output, .. } = docker.start_exec(&exec.id, None).await? {
    while let Some(chunk) = output.next().await { /* LogOutput */ }
}
```

**Stop, pause and remove** (lifecycle):

```rust
use bollard::query_parameters::{StopContainerOptionsBuilder, RemoveContainerOptionsBuilder,
                                ListContainersOptionsBuilder};
docker.stop_container(&name, Some(StopContainerOptionsBuilder::default().t(120).build())).await?;
docker.pause_container(&name).await?;                              // keep RAM, freeze CPU
docker.remove_container(&name, Some(RemoveContainerOptionsBuilder::default()
    .force(true).v(false).build())).await?;                        // keep the volume unless resetting
let ours = docker.list_containers(Some(ListContainersOptionsBuilder::default().all(true)
    .filters(&HashMap::from([("label", vec!["dev.gravity.home=<daemon_id>"])])).build())).await?;
```

**Fit with hermesd.** hermesd is Tokio 1.53 and Axum 0.7 on hyper 1.x
(`Cargo.lock`). bollard 0.21 is hyper 1 / hyper-util and Tokio, so it adds no second
HTTP stack. The probe resolved hyper 1.12. The `pipe` feature brings both
`hyperlocal` (Unix) and `hyper-named-pipe` (Windows). Streams are
`futures::Stream`, so they fit the daemon's existing `tokio::spawn` workers. Put all
Docker calls behind the `ComputerProvider` trait so tests use a fake, the way
`runtime/double.rs` stands in for Claude Code.

---

## 6. What breaks in Gravity today when a bot runs inside a VM

How a bot runs now, briefly:

- **Supervisor and runtimes.** The supervisor (`crates/hermesd/src/supervisor/lifecycle.rs:10`)
  builds a `BotSpec` and hands it to a `RuntimeAdapter` (`runtime/mod.rs:126`).
  - `pty` spawns `claude` in a `portable-pty` (`runtime/pty.rs:39`).
  - `codex` starts a per-bot Codex App Server on a random loopback port with a
    capability token, and attaches the native TUI with `--remote`
    (`runtime/codex/transport.rs:20`).
  - `double` is the deterministic test runtime.
  - `start_session` records the session's root PID for bus identity
    (`supervisor/start.rs:38`). Crashes restart with backoff, and `reconcile`
    brings every live bot up on every tick.
- **Workspace.** `~/.thehermes/projects/<p>/bots/<b>/{bot.json, system.md, mcp.json,
  browser/profile, workspace/{CLAUDE.md, FACTS.md, .claude/settings.json}}`, plus the
  project's shared `artifacts/` and git worktrees under the owner's `trusted_paths`
  (`docs/architecture.md`, "Filesystem layout").
- **Talking back to the daemon.**
  - MCP goes through `hermesd bus-proxy --endpoint <unix socket | named pipe>`, a stdio
    MCP server the session starts as its own child (`bus_auth/proxy.rs`).
  - Hooks go through `hermesd hook <event> --endpoint …` (`bus_auth/hook.rs`).
  - The daemon names the caller from the kernel: `peer_cred` on Unix
    (`bus_auth/ipc.rs:91`), the pipe client on Windows. It then walks the process
    tree up to a recorded session root, at most 8 levels, checking start times so a
    reused PID can't join (`bus_auth/session.rs`, `bus_auth/os.rs`).
  - Bearer tokens over `POST /mcp` are phase 1 and are still accepted by default
    (`bus_auth.rs:36`).
  - Deliveries go the other way, into **Claude Code's own inbox socket**, whose path
    the SessionStart hook reports (`channel.rs`).
- **Linked computers.** A peer daemon over `ws://…/peer` with a peer token
  (`peer/socket.rs`). Its bots appear here as stand-ins. Messages, tasks, artifacts,
  chat, terminal (`peer/term.rs`) and browser (`peer/browser.rs`) are relayed.
  **Owner acts forwarded from a linked computer are refused** (`peer/owner_trust.rs`,
  H-301/H-303), because "a peer token is a file bots can read".
- **Owner actions.** A bot proposes an exact command. Only the owner runs it, with
  the app's ticket or a paired device. A loopback connection from a bot's own session
  is refused (`owner_action.rs`, `ws/owner_actions.rs:116`).
- **Agent browser.** The Playwright MCP drives a headless Chrome on
  `<bot>/browser/profile`. The daemon reads `DevToolsActivePort`, lists tabs and
  streams `Page.startScreencast` JPEG frames (`browser/cdp.rs`, `browser/streams.rs`).
  Input is CDP `Input.dispatch*` (`browser/input.rs`). Protocol:
  `watch_browser`/`browser_frame`/`browser_input` (`docs/protocol.md:220`).
- **Secrets.**
  - `~/.thehermes/secrets/*.token` (`secrets.rs`).
  - The bot's bus token is in the session environment only in phase 1
    (`bus_auth::bearer_in_env`).
  - Codex gets a fresh App Server token per session.
  - Claude/Codex sign-in is the owner's own, on the host.
- **Platforms.**
  - macOS (launchd service) and Windows (Task Scheduler, Job Objects, named pipes,
    ConPTY) ship.
  - Linux builds and tests in CI (`.github/workflows/tests.yml` runs on ubuntu-24.04),
    and `bus_auth/os.rs` reads `/proc`. But there is **no Linux release artifact and no
    Linux service manager**: `service.rs` is launchd, and `service/windows` is the
    Windows one.

Each assumption that breaks when the bot's processes run in a guest:

| # | Assumption today | Where | What happens in a VM |
|---|---|---|---|
| B1 | **Bus identity by process lineage**: the daemon sees the caller's PID and walks to the session root | `bus_auth/session.rs`, `os.rs`, `ipc.rs:91` | The guest's processes aren't in the host's process table. A Unix socket can't be shared from a macOS host into a container in Docker's Linux VM, so `peer_cred` names a proxy process or nothing. **Identity has to come from somewhere else.** |
| B2 | The bus endpoint is a local Unix socket (`<home>/run/bus.sock`, mode 0600) or a per-user named pipe | `bus_auth/ipc.rs:23` | Unreachable from the guest. |
| B3 | `mcp.json` and the hooks run **`current_exe()`**, the host's absolute `hermesd` path | `bus_auth.rs:233` (`proxy_command`) | No such binary in the guest, and the wrong OS and architecture. |
| B4 | **Deliveries are written into Claude Code's inbox socket**, at a path the SessionStart hook reports | `channel.rs:27`, `supervisor/deliver.rs` | The socket is inside the guest. The daemon can't connect to it, so nothing is ever delivered. |
| B5 | **The PTY is spawned locally** (`portable-pty`), and terminal attach and resume read the daemon's ring buffer | `runtime/pty.rs:39`, `ws/terminal.rs` | The process has to run in the guest, with its PTY there. A `docker exec` PTY works for a Linux *container*, but for a dockur VM `exec` lands in the QEMU container, **not the guest OS**. |
| B6 | **Transcripts are read from `~/.claude/projects/<mangled absolute workspace>`** on the host (activity, chat, routine-completion matching) | `activity.rs:72`, `config.rs:158` (`user_home`) | They are written in the guest's home under the guest path's mangling. Activity previews, the chat view and **routine completion** (matched to a run marker in the transcript) all stop. |
| B7 | Codex's App Server is on host loopback, and `codex-observations.jsonl` is "beside the workspace" | `runtime/codex/transport.rs:20`, `docs/bot-runtimes.md` | Both are in the guest. |
| B8 | The agent browser is found by reading `<profile>/DevToolsActivePort` and dialing CDP on host loopback | `browser/cdp.rs:24` | The profile and the port are in the guest. Screencast, tabs and Take control stop. |
| B9 | Workspace trust is written into the host's `~/.claude.json`, with its lock | `paths/trust.rs` | The guest's Claude Code has its own `~/.claude.json`. Without trust, the trust dialog blocks the unattended PTY. |
| B10 | Absolute host paths are everywhere: `--mcp-config`, `system.md`, artifacts, `trusted_paths`, `<repo>-wt-*` worktrees, the scratch env, the guard hook's `--home` | `supervisor/lifecycle.rs:107`, `workers/scratch`, `bot_permissions` | None of them exist in the guest, and Windows guests use different separators and drive letters. The repos aren't there unless cloned. |
| B11 | **Hooks and bus share one daemon port, and the policy MCP allowlist is `http://127.0.0.1:49777/mcp`** | `ops/hermesd.example.toml` | From a guest, the host is `host.docker.internal`, `host.lima.internal` or the gateway, not 127.0.0.1. A policy-managed guest would drop the server. |
| B12 | **Loopback means this machine.** The owner-action check looks up the PID behind a loopback TCP client; peers and remote clients are "on the tailnet" | `ws/owner_actions.rs:116`, `ws/peers.rs:170` | **Measured:** a container under colima reaches services bound to the Mac's `127.0.0.1` through `host.lima.internal` / `host.docker.internal` / `192.168.5.2` (§6.1). A guest can therefore reach hermesd's `/ws`, `/mcp` (bearer phase 1), `/hook` and `/peer` on 49777. Today nothing a guest holds would authenticate there, but the "loopback ⇒ local and trusted" intuition is wrong once VMs exist. |
| B13 | Owner acts forwarded across a peer link are refused, in **both** directions | `peer/owner_trust.rs` | If a VM is a linked computer, the owner on the host **can't type into the VM bot's terminal or take its browser**, the very "take over" this feature is for. A VM peer needs an *asymmetric* trust: the guest trusts its host, and the host trusts nothing owner-like from the guest. |
| B14 | Processes are reaped with POSIX process groups or Job Objects; the holders ledger tracks session process groups | `holders.rs`, `runtime/pty/windows.rs` | Meaningless across the VM boundary. Stopping a bot must also stop or pause its VM. |
| B15 | Bus guidance and the system prompt assume one filesystem with the project's artifacts readable by path | `paths/prompt_sections.rs` | Paths in messages from a VM bot aren't readable on the host, and the reverse. The linked-computer wording already handles this ("paths … outside a transferred artifact are not readable here"). |
| B16 | Terminal attach has no input lease, and every `control` client can type | `ws/terminal.rs:1` | Unchanged, but a desktop also has a mouse and a screen, and **two hands on one desktop** (bot and owner) need the "watch / control" switch the browser already has. |
| B17 | Deadlines, leases and backoff use wall clocks, and peers compare timestamps | `peer` task mirroring | A paused or suspended VM's clock drifts; QEMU guests lose time across host sleep. Deadlines keep the sender's timestamp, so drift mainly hurts guest-side logs and leases. Run time sync in the guest. |
| B18 | Each bot is one Unix user's processes on the host | `docs/architecture.md`, "Trust boundaries" | This is the improvement, not a break: a bot in a VM can no longer read other bots' tokens, the owner's files or `~/.thehermes/secrets`. **But every host process (other bots included) that can reach the Docker socket can control every VM.** |

### 6.1 Light experiments run for this report

All of these were cheap and safe. No dockur image was pulled, nothing was installed
in a guest, and colima's configuration was not changed.

- **colima:** `colima status` reports it running on macOS Virtualization.framework,
  arch aarch64, docker runtime, virtiofs mounts. `colima list` shows 4 CPUs, 6 GiB RAM
  and a 40 GiB disk.
  - `~/.colima/default/colima.yaml` has `vmType: vz` and **`nestedVirtualization: false`**.
  - Inside the VM, **`/dev/kvm` does not exist**, `/proc/cpuinfo` has no vmx/svm, and
    the kernel ships a `kvm` module that has nothing to drive.
- **Docker:** `docker version` shows server 29.5.2, API 1.54, linux/arm64. The current
  context is `colima` (`unix:///Users/…/.colima/default/docker.sock`). The Docker
  Desktop context exists, but its socket is absent (it isn't running).
- **Image metadata only** (`docker manifest inspect -v`, nothing pulled), compressed sizes:

  | Image | amd64 | arm64 |
  |---|---|---|
  | `dockurr/windows` | 239 MB | 165 MB |
  | `dockurr/windows-arm` | 169 MB | 165 MB |
  | `dockurr/macos` | 161 MB | none (amd64 only) |
  | `qemux/qemu` | 178 MB | 142 MB |
  | `qemux/qemu-arm` | 144 MB | 141 MB |
  | `linuxserver/webtop:ubuntu-xfce` | 1280 MB | 1245 MB |
  | `kasmweb/ubuntu-noble-desktop:1.18.0` | 4247 MB | 2531 MB |

- **bollard 0.21.1 probe** (`…/scratchpad/bollard-probe`). It found the endpoint from
  the Docker CLI's current context, pulled `alpine:3.20` with progress events
  (3.7 s, 18 events), and created a container with labels, a loopback-only ephemeral
  port (8006 → `127.0.0.1:32768`), memory and CPU caps, `cap_add: NET_ADMIN` and a
  healthcheck. It then exec'd `uname -a`, saw `HEALTHY`, streamed the logs, and
  stopped and removed the container. Total under 10 s after the build.
- **Loopback reach:** a throwaway HTTP server bound to the Mac's `127.0.0.1` answered
  from an alpine container via `host.lima.internal`, `host.docker.internal` and
  `192.168.5.2` (B12). hermesd on this Mac listens on `127.0.0.1:49777` *and* on its
  tailnet address.

---

## 7. Design options and trade-offs

There are three decisions to make: which **computer provider** runs the guest, how the
bot **talks to the bus** from inside it, and how the **screen** reaches the owner
(that last one is §8).

### 7.1 Providers: one contract, several back ends

All providers sit behind a `ComputerProvider` trait. The supervisor doesn't care which
one it gets:

| Provider | Guest | Runs on | Isolation | Desktop and screen | Cost per bot | Notes |
|---|---|---|---|---|---|---|
| **P1. Desktop container**: our own Ubuntu image with Xvfb + XFCE + x11vnc + Chrome + Node + Claude Code/Codex + hermesd (the Bytebot / Anthropic-demo stack). `linuxserver/webtop` is a ready-made alternative, but it streams with Selkies, not VNC | Ubuntu 24.04/26.04 arm64/amd64 | Any Docker host: colima on every Mac today, Docker Desktop, Linux | Container: shared kernel, namespaces, cgroups. Not a VM | Real X desktop, standard RFB (VNC) on 5900 | ~1-2 GB RAM, ~1.3 GB image (shared by all bots), seconds to start | No KVM needed. **The only option that runs on this Mac today without changing colima.** |
| **P2. Ubuntu VM** (`qemux/qemu` `BOOT=ubuntu`, or `qemux/qemu-arm`) | Ubuntu, full VM | Linux with KVM; Windows 11 + WSL2; Apple M3+ with colima `--nested-virtualization` (arm64 guest) | Hardware VM (own kernel) | QEMU VNC (works from BIOS on), noVNC on 8006 | 2-4 GB RAM, 64 GB sparse disk, minutes to first boot | Interactive ISO install unless a cloud image + cloud-init is used. |
| **P3. Windows VM** (`dockurr/windows`; the arm64 image becomes windows-arm automatically) | Windows 11 / Server, or Windows 11 ARM | Linux with KVM; Windows 11 + WSL2 nested; Apple M3+ with nested colima (ARM Windows only) | Hardware VM | QEMU VNC, noVNC on 8006, RDP 3389 | 4+ GB RAM, 64 GB sparse disk, ~5-8 GB ISO download, about 15-40 min first boot (my estimate; dockur publishes no timing) | Unattended install, `/oem/install.bat`, `/shared` as `Z:`. |
| **P4. macOS VM, Apple-native** (`tart`, or `lima` vmType vz with a macOS guest) | macOS arm64 | Apple-silicon Macs only | Hardware VM (Virtualization.framework) | VNC from the framework (`tart run --vnc`) | 4-8 GB RAM, ~25-60 GB images | **The only licence-clean macOS guest.** Apple allows at most 2 extra macOS instances per Mac (§9). |
| **P5. macOS VM via dockur** (`dockurr/macos`) | macOS x86 (OpenCore) | Linux with KVM on x86 + AVX2 | Hardware VM | noVNC/VNC | 4+ GB RAM | amd64 only. The EULA allows only Apple hardware, so in practice only an **Intel iMac running Linux** qualifies. Manual install. |
| P6. Windows Sandbox / Hyper-V | Windows | The Windows PC | Hardware VM | RDP / VMConnect | | Sandbox is one at a time and throwaway. Hyper-V can't be driven through bollard. A possible later provider for the Windows PC. |
| P0. **No computer** (today) | n/a | Everywhere | Same user | Browser screencast only | 0 | Stays the default. |

**When is a full dockur VM the right tool?** When the bot's work needs *a different
OS* (Windows build and install testing, the 0.19 rule that "a Windows bot compiles and
installs it"), a **kernel-level boundary** (running untrusted installers, `sudo`,
system services, drivers), or a **disposable machine** to test a fresh install
against. When the bot just needs "a desktop with a browser and files", P1 is about ten
times cheaper in RAM, disk and start time. It also works on every Mac the owner has
today.

### 7.2 Bus identity and plumbing from inside a guest

**Option A: per-VM token plus a thin in-guest agent.** The daemon issues the
computer's bot a bearer token, injected into the guest. A small agent in the guest
(`hermesd computer-agent`) serves the PTY, relays hooks, delivers into the inbox
socket, and tails transcripts back. MCP goes to `http(s)://<host>/mcp` with the bearer.

- **For:** one SQLite and one supervisor on the host. The bot stays a normal local bot
  in every table. The token is safe *because* of the VM: the reason H-044 dropped
  bearers (any bot can read any other bot's token as the same OS user) doesn't hold
  when each bot has its own machine.
- **Against:** almost everything in B3-B9 has to be rebuilt as remote calls. That
  means a remote PTY protocol, a remote inbox, transcript sync for activity, chat and
  routine matching, a remote Codex App Server, remote CDP, and remote trust marking.
  It's a second, partial copy of the session machinery. hermesd would also have to
  listen where the guest can reach it (B11, B12), on a non-allowlisted URL.

**Option B: the computer is a linked computer.** A full hermesd runs *inside* the
guest in a "computer mode": one bot, no UI of its own, configured by the host. It is
paired with the host daemon as a peer. The host creates the bot *there* through the
existing `create_bot{peer_id}` frame, into a linked project. The bot on the host is a
stand-in (`bot.peer_id`).

- **For:** everything in B1-B10 keeps working unchanged *inside the guest*:
  - process lineage, the bus socket or pipe, `current_exe` hooks, the inbox socket;
  - the local PTY, transcripts, Codex, CDP, trust;
  - the allowlisted `127.0.0.1:49777` (the guest's own daemon).

  Every cross-machine feature already exists and is tested: messages and tasks with
  hop limits, artifacts, chat (`peer_chat`), terminal (`peer_terminal`), browser
  (`peer_browser`), boards (`board_call`), attachments and workers. The phone sees it
  through the host for free.
- **Against:**
  - A hermesd build for each guest OS and arch: Linux x86_64 and aarch64 are new
    release targets, while macOS arm64 and Windows x64 already ship.
  - A daemon and SQLite per VM, to upgrade with the host. Peer version negotiation
    exists (`peer_hello` `kanban:` contract).
  - Peer links today are **symmetric and owner-to-owner**, and B13 must change.

**Recommendation: Option B, with a new peer class `computer`.** The work it needs is
small and well contained, and almost all of it is about trust:

1. **Asymmetric trust.**
   - The *guest* takes owner acts forwarded by its host: terminal input, browser and
     desktop input, owner chat. The host can `docker exec` into it anyway, so the host
     holds strictly more power.
   - The *host* takes nothing owner-like from the guest. Today's H-303 refusals stay
     as they are for that direction.
   - The guest's `owner_trust::trusted` becomes per-peer: true only for its
     `computer_host` peer.
2. **A narrow surface on the host side** for a `computer` peer:
   - It exposes and serves **only its own bot's** stand-in.
   - It is refused `list_projects`, `link_project`, `unlink_project`, roster
     changes for other bots, `create_bot` (no spawning from the guest), and `machine`
     placement of workers onto the host.
   - `board_call` runs as the stand-in, under its board roles, as for any linked bot.
   - Attachment and artifact caps stay as they are.
   - A rogue guest (the bot has root there) can then do no more than its own bot
     already can over MCP.
3. **Automated pairing.** The host mints the peer token and the guest's config. It
   writes them into the guest through the provider: an env var or mounted file for
   P1, `/oem` or `/shared` for P3, `tart exec` or a mounted directory for P4.
   - **The host dials the guest**, at a loopback port Docker publishes. That needs no
     new listener on the host, and hermesd stays on loopback.
   - The guest's `/peer` accepts only that one token. The host refuses inbound `/peer`
     dials from a `computer` peer, so a guest holding the token gains nothing by
     dialing `host.docker.internal:49777` (B12).
4. **Credentials for the runtime.** `CLAUDE_CODE_OAUTH_TOKEN` (from
   `claude setup-token`) or `ANTHROPIC_API_KEY` for Claude Code, and `OPENAI_API_KEY`
   or a copied `auth.json` for Codex. They are injected as guest environment or files
   at provision time and kept in the host vault (`secrets/computer-<id>/`). Rotate
   them by re-provisioning.

Option A is still the better fit for one case: P1 containers on a **Linux** host,
where the host *can* see container PIDs and bind-mount the bus socket. That isn't
worth a second design; P1 uses Option B as well.

### 7.3 Workspace, transcripts and files

- **In the guest:** the bot's whole directory (`bot.json`, `system.md`, `workspace/`,
  `browser/profile`), its transcripts (`~/.claude/projects` in the guest) and its
  repos (cloned by the bot or seeded by the host).
- **Persistent:**
  - P1: the container's home is a **named volume** (`gravity-<bot>-home`, mounted at
    the user's home). It survives image upgrades, and the host can back it up with a
    throwaway container (`tar` of the volume).
  - P2/P3: the disk image lives in the provider's `/storage` volume.
  - P4: the tart VM bundle.
- **Between host and guest:**
  - Today's peer artifact transfer for `complete_task` results (16 MiB per file,
    48 MiB per message).
  - `attachment_fetch`/`attachment_push` for board files.
  - `read_file` frames for the app's Files panel.
  - Git for code.
  - Optionally, a **read-only** shared folder (`/shared` for dockur, a 9p or virtiofs
    mount for qemus, a bind mount for P1) carrying the project's `artifacts/` into
    the guest. It must be read-only: a writable host mount is a way out of the VM.
- **Transcripts never cross.** The host's chat, activity and steps for the bot come
  from the guest through `peer_chat`, as for any linked bot. Routine runs belong to
  the guest's daemon. The stand-in is what the host schedules against, and the
  existing forwarding covers it.

### 7.4 Lifecycle

| Bot event | Computer |
|---|---|
| Create with `computer: {provider, image, size}` | Host records a `computer` row and pulls the image (progress shown on the bot card). It creates the container or VM with labels `dev.gravity.bot=<id>`, `dev.gravity.home=<daemon_id>`, and starts it. Waits for health (VNC port answers, then the guest hermesd answers). Pairs. Sends `create_bot` to the guest. The stand-in appears. |
| Daemon start or reconcile tick | Lists containers by label (bollard `list_containers` with a label filter). Starts missing or stopped ones for live bots. Removes **orphans** whose bot is gone, after the retention window. |
| Restart bot | Restarts the bot's session *inside* the guest (forwarded `restart_bot`). The VM keeps running. |
| Stop / idle | Optional `suspend`: `docker pause` (P1/P2/P3 freeze, with RAM kept) or `docker stop` with a 2-minute grace for VMs (`stop_grace_period: 2m` in dockur's compose). |
| Reset computer (new owner action) | Delete the volume or disk and re-provision from the image or a base snapshot. The bot's identity, instructions and bus history on the host stay. |
| Archive or delete bot | Stop. After `retention`, remove the container, volume and disk. A "keep disk" option. |
| Host sleeps or reboots | `restart: unless-stopped` plus the reconcile tick. The peer link reconnects with backoff, as today. |

**Fresh vs reuse.** Long-lived bots keep one persistent computer. **Workers**, later,
get a fresh one cloned from a base: a P1 image, a qcow2 backing file for P2/P3, or
`tart clone`. **Snapshots:** dockur has none built in. Use `DISK_FMT=qcow2` and
`qemu-img snapshot` on a stopped VM (run through `docker exec` in the stopped volume's
container, or a helper container). For P1, use `docker commit` or volume tars. For
P4, `tart clone` is copy-on-write on APFS.

### 7.5 Resource limits and costs

- **Limits:** `HostConfig.memory`, `nano_cpus` and `pids_limit` on the container.
  dockur's `RAM_SIZE`/`CPU_CORES` inside it must fit under those (give the container
  RAM_SIZE plus about 0.5 GB). Disk: sparse images with `DISK_SIZE`; P1 volumes have
  no hard quota on colima.
- **New config caps** `[computers] max_running = 2`, `max_ram_gb`: the daemon refuses
  to start a computer that would exceed them, and says why on the bot card.
- **colima's VM is the real ceiling on Macs:** 4 CPUs and 6 GB today. Two Ubuntu
  desktops (about 1.5 GB each) fit. A Windows ARM VM (4 GB) fits only alone, or after
  `colima start --cpu 6 --memory 12` (the owner's call).
- **No money beyond licences.** Model usage is unchanged; it's the same subscription.
  A desktop's browser makes vision and screenshot-heavy turns more likely, so token
  use may rise.

### 7.6 Security boundaries

- **The VM isolates** the bot's processes, files, credentials it is handed, and
  installs. It can't read `~/.thehermes/secrets`, other bots' workspaces, the owner's
  home, Keychain, SSH keys or `~/.claude`.
- **A P1 container** is a weaker boundary (shared kernel), but it still isolates from
  the *macOS* host completely: on a Mac it lives inside colima's VM.
- **The VM does not isolate:**
  - **The network.** By default the guest has full outbound internet, and **it can
    reach the host's loopback services** (B12): hermesd, dev servers, databases (this
    Mac runs MongoDB, MinIO, MLflow and ZenML containers). Mitigations:
    - run computers on a dedicated Docker network with an egress policy (an iptables
      rule in the colima VM dropping traffic to `192.168.5.2`, except what is needed);
    - keep phase 2 (`bot_bearer = "refuse"`) so `/mcp` takes no bearer from anyone;
    - require tokens on everything on 49777 (already true).
  - **The Docker socket.** Any process that can reach the Docker socket controls
    every computer. On a Mac that is every bot that runs *on the host*, as the
    owner's user. Computers protect the host from their bots, not bots from each
    other's host-side peers. Keep host-side bots off the socket with the guard hook's
    deny list and the auto-mode environment.
  - **What the bot is given:** the runtime credential and the bot's own peer token.
    A root bot can read both and use the Claude/Codex token elsewhere. Same as today.
  - **Shared folders.** Mount them read-only.
  - **Viewer ports.** Publish VNC/noVNC/RDP/KasmVNC only on `127.0.0.1` with random
    host ports, and give each a random password the daemon holds. Otherwise any host
    process (or LAN host, if bound wide) could watch or drive the desktop.
  - The dockur defaults `USERNAME=Docker`/`PASSWORD=admin` must be replaced with
    generated values.

### 7.7 Failure model

| Failure | Detected by | Effect | Recovery |
|---|---|---|---|
| Docker not installed or not running | bollard `ping` fails at start or on create | Computer bots can't start (state `crashed`, reason "Docker isn't running") | Retry on the tick. The app links to setup. Non-computer bots are unaffected. |
| No `/dev/kvm` (macOS colima without nested virtualization, Windows 10, VPS) | Capability probe: start a tiny container with `/dev/kvm` mapped, or read `docker info`, before offering P2/P3 | The provider picker hides VM guests and says why | Documented host steps (§3) |
| Image pull fails or is slow | `create_image` stream error | Provisioning paused with progress | Resume. Pulls are idempotent. |
| First boot hangs (Windows "Copying files 0%", dockur #1156) | No VNC or guest-hermesd health within N minutes; noVNC shows the screen | Stuck in `provisioning` | The owner watches in the Computer tab. Retry with `DISK_FMT=qcow2`, or reset. |
| Guest daemon down or unreachable | Peer link down (`last_seen_at`) | Stand-in offline, deliveries `NotReady` (wait, no attempts spent; existing behaviour) | Restart the container. Show the desktop so the owner can see why. |
| Guest out of disk or RAM | Guest daemon health, container OOM-killed (`inspect.state.oom_killed`) | Bot crashes or restarts | Grow `DISK_SIZE` (dockur supports it), raise RAM, or reset |
| Host sleep or clock drift | Peer reconnect | Leases expire and redeliver (at least once, idempotent) | Time sync in the guest |
| Orphans after a crash mid-provision | Label scan on the tick | Wasted resources | Remove unlabelled-bot containers after retention |
| Docker endpoint changes (colima ↔ Docker Desktop) | Endpoint resolution per start | Computers "missing" | Containers belong to an engine. Record the engine id (`docker info` `ID`) on the computer row and refuse to re-provision silently on another engine. |
| Guest compromised | n/a | Contained to the guest and what it reaches over the network (above) | Reset computer. Revoke its peer token. |

---

## 8. The Computer tab: from a browser to the bot's whole desktop

### 8.1 What the Browser tab is today

- **Desktop app.**
  - `apps/desktop/src/components/bot/BotTabs.tsx` offers `browser` when the daemon
    advertises `bot_browser` (or `peer_browser` for a stand-in).
  - `BotView.tsx:87` starts `useBrowserWatch` as soon as a bot is selected, so the
    tab is live on open.
  - `BrowserPane.tsx` renders:
    - a `ControlBar` with *Take control* / *Give back control*;
    - a `TabStrip` (with "Follow bot");
    - a `BrowserScreen`, which is an `<img>` of the newest base64 JPEG
      `browser_frame`, or `BrowserControl`, which maps mouse and keys
      (`browserInput.ts`: `pagePoint`, `modifierMask`, Windows key codes) to
      `browser_input`;
    - a `BrowserActivityList`: every browser action, grouped by the turn's request.
  - For a linked bot it says that control happens on its own computer (H-303).
- **Daemon.**
  - `browser/cdp.rs` finds Chrome from `DevToolsActivePort`.
  - `browser/streams.rs` keeps one shared stream per bot and tab: a `watch` channel
    holding only the newest frame, polling stops when the last viewer leaves.
  - `browser/input.rs` dispatches CDP input.
  - `ws/browser.rs` serves `watch_browser`/`unwatch_browser`/`list_browser_activity`.
  - `peer/browser.rs` relays `browser_watch`/`browser_feed` over a peer link, at
    newest-frame-only and at most every 150 ms (`FRAME_INTERVAL`).
- **Phone** (`~/Developer/gravitiOS/TheHermes/UI/BrowserPane.swift`). The same
  requests, **watch-only** (no `browser_input` anywhere in the iOS code), with a
  full-screen zoom, tabs and the activity list.

### 8.2 What the Computer tab is

The `browser` tab is **renamed `computer`** for a bot that has a computer. A bot
without one keeps the Browser tab exactly as today, so nothing regresses. The tab has:

1. **The desktop**, live and scaled to fit, with a status line: provider and OS,
   *running / starting / installing (n%) / paused / offline*, and resolution.
2. **Watching ↔ Take control**, the same switch and wording as the browser.
   - While controlling, the owner's pointer and keys go to the desktop. The bot may
     act too, as in today's browser control.
   - Optional (phase 3): *pause the bot while I control*. The daemon tells the bot
     through a bus note, and its computer-use tools refuse to act until control is
     handed back.
3. **A side panel** with:
   - **Browser activity** (unchanged: the guest's agent-browser log, relayed);
   - a **Tabs** view of the bot's Chrome inside the desktop (unchanged
     `watch_browser`, which still streams one tab at page resolution and is handy on
     the phone);
   - **Files** (existing `read_file` relay).
4. **Computer actions** (owner-driven, `control` plus device or ticket): Restart
   computer, Pause, Reset to clean image (with confirmation), Open in a separate
   window, Copy RDP/VNC details (local only).

The bot's Playwright MCP stays as it is. In a computer it is configured
**`headless = false`**, so its Chrome windows appear on the desktop the owner watches.
The CDP screencast, tabs and activity keep working, because they come from the guest
daemon and reach the host through `peer_browser`. The bot can *also* get desktop
computer-use tools for apps outside the browser (phase 3): screenshot, click, type,
key, through the guest daemon's own VNC or X connection, the way Bytebot and the
Anthropic demo do it.

### 8.3 Ways to stream the desktop into the tab

| | (a) dockur/qemus noVNC (8006) in an iframe or webview | (b) **RFB proxied through hermesd** (raw RFB tunnelled over the control-plane WS, rendered with noVNC's `RFB` class in the app) | (c) KasmVNC / Selkies / WebRTC (Neko) for Ubuntu | (d) **VNC → frames bridge** in hermesd, reusing the `browser_frame` path |
|---|---|---|---|---|
| Works for | dockur and qemus VMs only | Anything that speaks RFB: QEMU's VNC (every dockur/qemus VM, from BIOS on), x11vnc/TigerVNC in our Ubuntu image, tart `--vnc`, lima `display: vnc` | Ubuntu containers only (webtop is Selkies since 2025-06; KasmVNC is *not* RFB-compatible) | Same as (b) |
| Auth | Its own (`PROTECT=Y` basic auth) on a port **the app must reach directly**. Not through the daemon, not over a peer link, not from the phone unless the port is exposed | **Only hermesd connects** to the guest's VNC (loopback, random password). Clients use the existing WS grants (`read` to watch, `control` + device or ticket to drive) | Its own auth and ports. WebRTC needs UDP or TURN and can't go through the daemon's WS | Same as (b) |
| Latency and quality | Good locally | **Best**: incremental RFB updates (Tight/ZRLE/CopyRect), noVNC renders on a canvas; LAN about 20-50 ms | Best for video (H.264/WebRTC) | Coarse: decode, re-encode JPEG, ~5-10 fps, 100-200 ms; fine for watching |
| Phone | Webview in iOS; separate auth | Needs an RFB client on iOS: noVNC in a `WKWebView` fed by the daemon's WS, or a Swift RFB decoder | Webview, direct connection | **Works today**: the phone already renders `browser_frame` JPEGs |
| Across linked computers | No | Yes, by relaying the byte stream over `/peer` (a new `computer_rfb` frame), though the multiplexed link adds latency | No | **Yes**: copy `peer/browser.rs` (newest-only, ≤ every 150 ms) |
| Take control | Built in, no owner proof | `viewOnly` toggles in noVNC; the daemon drops input unless `control` + device or ticket | Neko has watch and control roles (`can_host`) | `computer_input` with today's `browser_input` event shapes, mapped by the daemon to RFB `PointerEvent`/`KeyEvent` (key → X keysym) |
| Effort | Smallest, but a dead end | Medium: a byte relay plus noVNC (MPL-2.0) in the app | High, and a second stack for Ubuntu only | Medium: an RFB client in Rust (`vnc-rs`, MIT/Apache), a JPEG encoder, reusing streams, frames, viewers and the peer relay |

Sources:

- noVNC embedding and API (`view_only`, `resize=scale`, `viewOnly`,
  `credentials`): https://novnc.com/noVNC/docs/EMBEDDING.html,
  https://novnc.com/noVNC/docs/API.html
- KasmVNC: https://github.com/kasmtech/KasmVNC
- webtop on Selkies: https://docs.linuxserver.io/images/docker-webtop/
- Neko: https://neko.m1k1o.net/docs/v3/configuration/authentication
- vnc-rs: https://github.com/HsuJv/vnc-rs
- IronRDP (for RDP later): https://github.com/Devolutions/IronRDP

**Recommendation: (d) for the MVP, then add (b) for control on the desktop app.**

- **(d) reuses almost everything.** The frame and stream model (`streams.rs`), the
  viewer slots, the phone UI, the peer relay, and the input path's grants and
  owner-driven checks (`ws/owner_auth.rs` `OWNER_DRIVEN` gains `computer_input`).
  It works the same for Ubuntu containers, dockur VMs and tart, because QEMU, x11vnc
  and Virtualization.framework all speak RFB.
- **(b) adds smooth control.** The daemon relays the RFB bytes after doing the VNC
  auth itself. It strips the security handshake and offers "None" to the client, so
  the client never learns the password. The app's noVNC renders them. The phone stays
  on (d), unless the owner wants phone control in phase 3.
- **Rule out (a)** except as a debugging link: it bypasses the daemon's auth and
  can't reach the phone or a linked computer.
- **(c)** is only worth it if (b) proves too slow for video-like use, which is
  unlikely for agent work.

**Where the RFB server lives:**

- **P1 Ubuntu container:** our own image runs Xvfb + XFCE + x11vnc on `127.0.0.1:5900`
  inside the container, published to host loopback with a password. This is the
  Bytebot and Anthropic-demo stack.
- **P2/P3 dockur/qemus:** QEMU's own VNC server (`DISPLAY=vnc`, port 5900), which also
  shows install and boot screens. The 8006 noVNC page can stay unpublished.
- **P4 tart:** `tart run --vnc` / `--vnc-experimental`.

**Desktop computers on a linked computer.** A computer that runs on the Windows PC's
Docker belongs to a bot hosted by the PC's daemon, which is the computer's host.
The Mac sees it as a stand-in, and the frames come over the existing peer link.
Control from the Mac stays refused by H-303 until signed owner approvals exist, so
the phone paired with the PC, or the PC itself, can take control. This is today's
browser rule, unchanged.

**Protocol additions** (additive; capability `bot_computer`, plus `peer_computer`):

- `watch_computer{bot_id}`. Pushes `computer_state`
  `{bot_id, provider, os, state, progress?, width, height}` and `computer_frame`
  `{bot_id, data (JPEG base64), width, height, seq}`, newest-wins like
  `browser_frame`.
- `unwatch_computer`.
- `computer_input{bot_id, event}`, where `event` is today's `browser_input` event
  (mouse, wheel, key, text). It requires `control` plus a device or ticket.
- Phase 2: `open_computer_stream{bot_id, mode: view|control}`, which returns a stream
  id. Binary WS frames then carry RFB bytes each way. `ws/binary.rs` already exists
  for binary pushes.
- `computer_action{bot_id, action: restart|pause|resume|reset}`, owner-driven.

**Latency budget.**

- (d) locally: the RFB update over loopback, a 1280×800 JPEG encode at q≈70 (about
  10-20 ms on an M-series CPU), a WS push, and the img decode. That gives about
  60-120 ms per frame, at 5-10 fps.
- Over a peer link, frames are capped at one every 150 ms (as for the browser).
- (b) is bounded by RFB itself.

---

## 9. Recommended MVP and phases

### MVP: an Ubuntu computer for one bot, on the bot's own host, watched and taken over from the Computer tab

The MVP covers:

- **An Ubuntu desktop container (P1)**, our own image, on the host's Docker through
  bollard. It runs on this Mac **today** and on the Windows PC. No KVM, no licences.
- **A guest hermesd in computer mode** inside it, paired automatically as a
  **`computer` peer** with asymmetric trust (§7.2). The owner creates the bot *in*
  the computer; the host keeps the stand-in. Chat, terminal, browser activity, tasks
  and boards then work through the existing peer relays.
- **The Computer tab**: watching through the VNC → JPEG frames bridge (§8.3 d), and
  Take control through `computer_input`, on the desktop app. The phone gets
  watch-only for free (the same frames).
- **The bot's Chrome runs headed** on that desktop. The browser activity list and
  per-tab view stay as a side panel.
- **Owner-only creation and opt-in per bot.** A computer is created from the app
  ("New bot → with its own computer"), not by bots over MCP.

The MVP leaves out: full VMs, Windows, macOS, the RFB tunnel, snapshots, workers with
computers, control across linked computers, and phone control.

**Files and modules it touches** (new files keep the 400-line rule):

| Area | Files |
|---|---|
| New module | `crates/hermesd/src/computer.rs` plus `computer/`: `docker.rs` (endpoint resolution: config, `DOCKER_HOST`, CLI context, well-known sockets, `npipe`), `provider.rs` (`ComputerProvider` trait and a fake for tests), `ubuntu.rs` (P1 container spec), `reconcile.rs` (label scan, start, orphans, engine id), `pairing.rs` (peer token, guest config, host dials the published port), `vnc.rs` (RFB client via `vnc-rs`, newest-frame JPEG, shared streams like `browser/streams.rs`), `input.rs` (`browser_input` events → RFB pointer and keysym events) |
| Dependencies | `crates/hermesd/Cargo.toml`: `bollard = { version = "0.21", default-features = false, features = ["pipe"] }`, `vnc-rs`, a JPEG encoder (`image` with jpeg only, or `turbojpeg`); `Cargo.lock`; `third-party` notices |
| Config | `crates/hermesd/src/config.rs`: `[computers] enabled, docker_host, ubuntu_image, max_running, ram_gb, cpus`; `ops/hermesd.example.toml` |
| Schema | `crates/bus/src/schema/` (new migration): table `computer(bot_id, provider, image, container, engine_id, state, vnc_port, peer_port, created_at, reset_at)`, and `peer.kind TEXT DEFAULT 'daemon'` (`'computer'`); `crates/bus/src/types/entities.rs` (`Bot.computer` summary for clients); `crates/hermesd/src/db/` accessors |
| Peer trust | `crates/hermesd/src/peer/owner_trust.rs` (per-peer: the guest trusts its `computer_host`); new `peer/computer.rs` (host refuses `list_projects`, `link_project`, `create_bot`, foreign rosters and inbound dials from a computer peer); `peer/socket.rs` (dial-only for computer peers); `peer/roster.rs`, `peer/receive.rs` (scope to the one bot) |
| Guest mode | `crates/hermesd/src/main.rs` (`--computer` or `[computer] host_peer`: no UI pairing, one project, headed browser); `browser/setup.rs` (`headless = false` in a computer); `paths/prompt_sections.rs` ("you have your own Ubuntu computer…") |
| Supervisor | `supervisor/lifecycle.rs`: a bot with a computer is a stand-in (already skipped by `start_bot`); `botmgmt.rs` and `ws/entities.rs` (`create_bot{computer}` → provision, pair, `create_bot` on the guest) |
| Control plane | new `ws/computer.rs` (`watch_computer`, `unwatch_computer`, `computer_input`, `computer_action`); `ws/dispatch.rs`; `ws/owner_auth.rs` (`OWNER_DRIVEN` += `computer_input`, `computer_action`); `app.rs` (`CAPABILITIES` += `bot_computer`); `docs/protocol.md` |
| Guest image | `ops/computer/ubuntu/Dockerfile` (Ubuntu 24.04, Xvfb, XFCE, x11vnc on 127.0.0.1:5900 with a password from env, Chromium, Node 22, Claude Code, Codex, hermesd), `entrypoint.sh` (writes `hermesd.toml` and secrets from env and mounted files, then runs hermesd); a CI job that builds multi-arch and pushes to GHCR, pinned by digest in config |
| Release | `.github/workflows/release.yml`: **new Linux targets** `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` for hermesd (they build and test in CI already; `openssl-sys` is in the lockfile, so use glibc, not musl, at first) |
| Desktop app | `apps/desktop/src/components/bot/BotTabs.tsx` (`computer` tab instead of `browser` when `bot.computer`); new `components/computer/ComputerPane.tsx`, `useComputerWatch.ts`, `ComputerScreen.tsx` (generalize `BrowserScreen`/`BrowserControl`/`browserInput.ts`, which already map pointer and keys); `protocol/agents.ts` and `protocol/api.ts`; `test/fakeDaemon.ts`; Ladle stories and visual baselines |
| Phone | `~/Developer/gravitiOS/TheHermes/UI/ComputerPane.swift` (watch-only, reusing `BrowserPane`'s frame view and zoom); `Core/Agents.swift`, `Core/AppStore.swift` |
| Tests | `crates/hermesd/tests/computers.rs`: a fake provider, two in-process daemons (host and guest on the `double` runtime) as `tests/peer_bots.rs` does, covering pairing, the trust asymmetry, refusals of a rogue computer peer, frames and input; an opt-in `#[ignore]` test against a real Docker and the real image |
| Docs | new `docs/computers.md`; `docs/peer-bots.md` (the computer peer class); `docs/bot-browser.md` (headed in a computer); `docs/architecture.md` (trust boundaries) |

**Acceptance:** the owner creates "Researcher" with a computer on this Mac. The bot
appears and answers chat. The Computer tab shows its XFCE desktop. The bot opens
Chrome and browses, and the owner watches it live on the desktop app and the phone.
*Take control* signs in to a site and *Give back control* hands it back. The bot
delegates and replies over the bus like any bot. Then:

- Restart the daemon: the computer and its files survive.
- Reset the computer: a clean desktop, same bot.
- A shell in the guest that impersonates the guest daemon can't list or link the
  host's projects, or reach another bot.

### Phases

1. **MVP** (above). Ubuntu container computers, one host, watch and control on
   desktop, watch on phone. Rough size: 3-4 weeks for one engineer or bot team,
   mostly in pairing, trust, the guest image and the VNC bridge.
2. **Full VMs where KVM exists, and Windows.**
   - A capability probe: start a tiny container with `/dev/kvm` mapped, at startup.
   - Ubuntu as a real VM (`qemux/qemu`, a cloud image plus cloud-init so it is
     unattended).
   - **Windows via `dockurr/windows` on the Windows PC.** `/oem/install.bat` installs
     Git, Node, Claude Code and the hermesd x64 service and drops the pairing file.
     QEMU VNC (`DISPLAY=vnc`) feeds the same bridge. Install progress shows in the
     Computer tab from the first BIOS screen.
   - This is what lets a Windows bot compile and install Gravity on a clean Windows
     each release (`required_machines` keyed by label, `docs/kanban-rework.md` K8).
   - Resource caps in the UI, and pause on idle.
3. **Smooth control and computer use.**
   - The RFB tunnel over the control-plane WS with noVNC in the desktop app (§8.3 b).
   - *Pause the bot while I control*.
   - Desktop computer-use tools for the bot (screenshot, click, type) through its
     guest daemon.
   - Reset and snapshots: `DISK_FMT=qcow2` plus `qemu-img snapshot`; volume tars for
     containers.
   - Phone take-control, if the owner wants it.
4. **macOS on Apple silicon** via a `tart` (or lima) provider: a second
   `ComputerProvider` implementation, not Docker. At most two per Mac. `tart exec`
   with the guest agent for provisioning, `--vnc-experimental` for the bridge,
   `--dir …:ro` for read-only shares.
5. **Workers and fleets.**
   - Fresh computers from a base for temporary workers (container image, qcow2
     backing file, or `tart clone`).
   - Placement across linked computers (a spawn on a full Mac goes to the PC's Docker).
   - Signed owner approvals (owner decision 6df14f7a) so control works across linked
     computers too.

**Optional phase 0, a one-day demo:** keep the bot on the host and point its
Playwright MCP at a headed Chrome inside an Ubuntu desktop container
(`--cdp-endpoint`, published to loopback), then show that container's desktop in
the Computer tab. It proves the screen pipeline (bollard, VNC, frames, tab) with no
peer or trust work. It is **not** isolation, though, and it shouldn't ship as "a
computer".

---

## 10. Risks and licensing

**Licensing.**

- **Windows guests.**
  - dockur "does not distribute Windows". The keys built into its answer files are
    "generic installation keys … for trial purposes and are not valid activation
    licenses".
  - An unactivated Windows 11 installs and runs, with a watermark and locked
    personalization. Using it that way long-term is the owner's licensing call: one
    Windows licence per persistent VM, a volume or VDA arrangement, or
    evaluation/Enterprise media for short-lived test machines. dockur takes the
    owner's key as `KEY`.
  - This needs the owner's decision (Q6).
- **macOS guests.**
  - The macOS SLA §2B(iii) allows "up to two (2) additional copies or instances" of
    macOS in virtual machines, on each Apple-branded computer **that is already
    running macOS**. Allowed purposes are development, testing, macOS Server, or
    personal non-commercial use (https://www.apple.com/legal/sla/; wording quoted at
    https://eclecticlight.co/2022/08/04/virtualisation-on-apple-silicon-macs-8-how-apple-limits-vms/;
    the current Tahoe PDF wasn't text-extractable, so recheck it).
  - **P4 (tart or lima on a Mac running macOS) fits squarely.** Apple enforces the
    two-VM cap.
  - **`dockur/macos` on non-Apple hardware** (the Windows PC, a Linux server) is
    outside the licence. dockur itself says "Only run this container on Apple
    hardware".
  - On an **Intel iMac running Linux** it is Apple hardware. But that iMac would no
    longer be "already running" macOS, so the clause arguably doesn't cover it. Treat
    it as not cleared (Q4).
  - Recommendation: macOS computers only through P4 on Apple-silicon Macs.
- **Ubuntu guests.** Free. Don't bake Google Chrome into a published image (its
  terms restrict redistribution). Ship Chromium or Playwright's Chromium (also the
  only choice on arm64), or install Chrome at first boot.
- **Components.**
  - dockur and qemus are MIT.
  - bollard is Apache-2.0. vnc-rs is MIT/Apache.
  - noVNC is MPL-2.0 (file-level copyleft, fine to bundle; keep its notices in
    `third-party`).
  - KasmVNC is GPL-2.0, so we avoid it.
  - Selkies is MPL-2.0. Neko is Apache-2.0.
  - tart is **FSL-1.1-ALv2**: internal use is fine, competing commercial offerings
    aren't; it becomes Apache-2.0 after 2 years.
  - Claude Code and Codex are used under the owner's own plans, as today.

**Technical risks.**

| Risk | Likelihood | Mitigation |
|---|---|---|
| **Trust model regression.** A computer peer gets owner-like powers on the host | Medium if done carelessly | The `computer` peer class, with an explicit allowlist of frames; tests that a rogue guest daemon is refused (in the MVP acceptance) |
| **Guests reach host loopback services** (measured, B12) | Certain by default | A dedicated Docker network with an egress rule blocking the host gateway; keep everything on 49777 authenticated; phase 2 bearer refusal |
| **The Docker socket is root over every computer** | Certain | Document it. Keep host bots off the socket (guard hook deny list); no socket mounts into guests, ever |
| RAM: colima's 6 GB caps this Mac at about 2 Ubuntu computers, or 1 Windows ARM | High | `max_running`, pause idle computers, ask the owner to size colima (Q5) |
| Disk: 64 GB sparse per VM grows; ISOs 5-8 GB | Medium | `REMOVE=Y`, qcow2, a shared base image, retention on delete, a disk floor check (the checks module already has one, H-283) |
| dockur moves fast (v6.00 → v6.06 in 3 months), and the WSL2 install hang | Medium | Pin image digests; an upgrade checklist; show the install screen so a hang is visible |
| Version skew between host and guest hermesd | Medium | The guest image is built from the same release; `peer_hello` contract negotiation; the host refuses a guest older than N−1 and offers "Update computer" |
| A second Linux daemon to maintain (no systemd service; container entrypoint) | Low | In P1 the container *is* the service. In P2 VMs, a systemd unit (new `service/systemd.rs` later) |
| Latency of the frames bridge feels laggy for control | Medium | Phase 3 RFB tunnel; Take control is rare (sign-ins, CAPTCHAs) |
| Clock drift in paused VMs | Low | Time sync in the guest; deadlines already use the sender's timestamps |
| Policy-managed Macs: the MCP allowlist | Low | The guest's own daemon serves `127.0.0.1:49777` inside the guest, so the allowlisted URL still matches |
| Docker Desktop vs colima: two engines on one Mac | Low | Record the engine id per computer; never silently re-provision on another engine |

---

## 11. Open questions for the owner

1. **Is "grob bot" xAI's Grok Bot?** Note that Grok Bot gives **one computer per
   account**, with a screen per bot and no isolation between bots. This design gives
   **one computer per bot**. Should there also be a cheaper *project computer* that
   several bots share, Grok-style?
2. **Which bots get a computer?** Opt-in per bot (assumed), or by default for new
   bots? May bots create bots with computers over MCP, or only the owner (assumed)?
3. **Computer tab for every bot?** Assumed: only bots with a computer get the
   Computer tab, and others keep Browser. Should Browser also be folded into
   Computer for all bots?
4. **Is the iMac Intel or Apple silicon, and would you put Linux on it?** With Linux
   it can host x86 Windows, Ubuntu VMs and (licence permitting, see §10) dockur/macos.
5. **May Gravity, or you, change colima?** `--nested-virtualization` for full VMs and
   Windows ARM on M3+ Macs, and more RAM and CPU (today 4 CPU / 6 GB). Or should
   computers use Docker Desktop on the Windows PC only?
6. **Windows licences.** Bring keys (`KEY`), accept unactivated test VMs, or keep
   Windows computers short-lived (fresh per release test)?
7. **Credentials in guests.** OK to inject a `claude setup-token` OAuth token or an
   API key, and a Codex key, into each computer? One shared token, or one per
   computer for revocation?
8. **Network policy.** Full internet for computers (assumed yes)? Block the host's
   loopback services by default (recommended)?
9. **Phone.** Watch-only in the MVP (assumed), or take-control from the phone as
   Grok Bot offers?
10. **Release scope.** OK to add Linux hermesd release artifacts and publish a
    `gravity-computer-ubuntu` image to GHCR? Both are new public build outputs.
11. **Persistence.** Keep a deleted bot's computer disk for the retention window
    (assumed), or delete it at once? Should workers get fresh computers (phase 5)?
12. **Placement.** When the Mac is full, should a computer be placed on the Windows
    PC's Docker automatically (the bot then runs there as a linked bot), or should
    you pick the machine?

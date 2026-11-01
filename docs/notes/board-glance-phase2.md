# 0.20.0 Board glance, phase 2: daemon and protocol work

Phase 1 (phone only) uses data the phone already has. Phase 2 needs the daemon changes below.
Findings are from origin/main @ 7505848b, 2026-10-16. Design: https://claude.ai/artifact/MGnBBnRiRnrnJLQkDKGSqj (page "Board glance deep dive").

## What exists today
- `create_project {name}` creates only the project row and folder: no board, no lead (`projectmgmt.rs:58-70`).
- The first board comes only from `board_enable` / auto-enable on `board_get`. It is always preset `simple_review`, board "Main" (`db/board.rs:96-161`, `Preset::FIRST`). `board_create` can't make a first board.
- The first local bot becomes lead automatically (`claim_first_lead`). A lead on a peer needs `set_project_lead` and then `role_set ROLE_LEAD`.
- The owner may send board structure requests (`board_enable`, `column_add/update/remove`, `role_set`, `board_update`) under the `approve` grant.
  - Every owner structure change DMs the lead, so edit columns before creating the lead.
  - Column ops chain by board `version`.
  - On a release-gated board, columns can only be renamed or reordered.
- Presets (`kanban/presets.rs`):
  - `simple`: To do / Doing / Done.
  - `simple_review`: todo / doing / review / done. Review reports category "doing".
  - `software`: Inbox / Ready / Doing / Review / Done.
  - `release_gated`: Inbox / Ready / Doing / Review / Verify / Awaiting owner / Deploying / Done.
- Repo: `set_project_repo {url, branch}` only records it. The daemon never clones into a chosen folder; workers clone into their workspace.
- Project description: none in the model (only boards and bots have one).
- Delete: `delete_project`, `project_delete_preview`; both are owner-driven and move leftovers to the Trash.
- Restart: `restart_bot {bot_id}` (capability `restart_bot`) and `clear_bot_session`.
- Overview: `ProjectRow` has `columns[]` (includes the hidden cancelled column, no kind or visible flag) but no bot list, no top attention row, and no bot-stopped kind.
  - Builder: `overview/build.rs::row()`, `overview/part.rs`.
  - Attention: weights in `attention.rs:31-54`, producers in `attention/rows.rs`.

## Additions (additive proto, gated by capability strings in app.rs)
1. **`BoardEnable.preset`** (optional string), plus optional `repeated ColumnSpec columns`. Capability `board_presets`.
2. **Project description** column: on `create_project` / `update_project` and in `project_view`. Capability `project_description`.
3. **`create_bot {lead: true}`**, which sets the project lead and, if a board exists, `ROLE_LEAD`. Works for peer bots too.
4. **`ProjectRow.bot_list`** (`BotBrief`: daemon_id, bot_id, name, avatar, runtime, state, status, computer_name, lead, state_reason, since), carried in `ProjectAttention.Part.bots`. Capability `overview_bots`.
5. **`ProjectRow.top_attention`**: sort here plus peer part rows and take the first. Capability `overview_top_attention`.
6. **`AttentionKind.BOT_STOPPED`**, plus `AttentionRow.bot_state`, for crashed / auth_failed / rate_limited / stopped. Action: `restart_bot`. Capability `attention_bot_stopped`.
7. **`Member.online` and `Member.last_seen_at`** from `Peer.last_seen_at` and `app.peers.is_online`.
8. **`ColumnCount.kind` and `ColumnCount.visible`**, so clients can drop hidden columns and spot review and done columns (simple_review reports Review as category "doing").
9. **Optional:** a "lead finished its first turn" signal for the creation progress step.
10. **Retroactive capability** for the 0.19 structure requests, or gate them on `server_version >= 0.19.0`.

Fixtures to update: `crates/bus/fixtures/home/*`, `crates/bus/tests/home_fixtures.rs` and `crates/bus/fixtures/board/messages/request.board_enable.json`. Then regenerate the desktop TS (`apps/desktop/src/protocol/gen`) and re-vendor `contract/` into flotilla-ios (`scripts/contract/vendor.sh`).

## Phone creation flow with today's calls
1. `create_project`
2. `board_enable` (with preset, once added)
3. column edits
4. `link_project` (if the lead runs on a peer)
5. `create_bot {name, runtime, peer_id?}`
6. `set_project_lead` + `role_set ROLE_LEAD` (peer lead)
7. `set_project_repo`

Do the board before linking: two projects that both have a board can't be linked.

## Owner decisions (2026-10-16)
- **Third template:** use the daemon's real `release_gated` board (Inbox · Ready · Doing · Review · Verify · Awaiting owner · Deploying · Done), shown as is in step 1. No new preset.
- **Repository:** build a clone step. Add `clone_project_repo {project_id, url, branch?, dest}`, owner-driven, with the destination confined to the user's home and the clone done through `safe_git`. Progress is pushed as `repo_clone {project_id, state: cloning|done|failed, error?}` and shown on the Creating screen. Step 4 keeps the folder field.
- **Description:** add a real project description field (schema, `create_project`/`update_project`, `project_view`), given to the lead as context.

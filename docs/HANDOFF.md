# Tray Session — Handoff

_Last update: 2026-10-03_
_Next agent: Claude Opus 5.5_

## Project

Tray Session — game session tracker for Windows.
Rust 2021, egui 0.28 + eframe 0.28, SQLite (rusqlite), sysinfo,
nvml-wrapper, winreq/winapi, reqwest, tray-icon, notify-rust, arboard,
rfd, open. Built as 7-Zip SFX installer.

Repo: F:\Programms\Tray Session python\game-session-tracker

## Where we are now

- **Stable candidate**: 0.7.36 (merged into Tray-session; publication pending)
- **Beta**: 0.7.33-beta.8 (app-Tray_session branch)
- **Working branch**: Tray-session, merge commit 91c248d (0.7.36 candidate)
- **Tests**: 193 passed, 7 ignored; clippy completes with existing warnings
- **Last major**: UI refresh, monitor-aware scaling, background GPU/session refresh

## Repo structure (essentials)

- src/ — Rust source
- src/ui/theme/ — color tokens, skins
- src/ui/layout/ — H1 layout engine (spec + presets + engine)
- src/ui/views/ — (skeleton, unused)
- src/ui/widgets/ — (skeleton, unused)
- docs/ — this file, tasks.md, bugs.md, licenses, ui_refactor_audit
  (archived)
- tools/ — build/publish scripts
- target/release/TraySession.exe — current build

## What's done (major milestones)

1. **Updater rewrite** (pipeline: mutex → reexec → wait_pid → download
   → atomic replace → spawn). End-to-end verified 2026-10-02.
2. **Update confirmation dialog** with silent-mode countdown and
   changelog from manifest.
3. **Theme tokens** centralized (ui/theme/tokens.rs). No hardcoded
   colors in widgets.
4. **Single-instance** via WinAPI mutex.
5. **C8**: build date auto-generated via build.rs.
6. **C9 v2**: tray "show window" via WinAPI (Minimized + ShowWindow).
7. **C11**: ComboBox channel/frequency saves correctly.
8. **H1 step 1-3b**: LayoutSpec + presets + engine split
   (build_nav/status/center) + nav wired into app.rs.

## What's next (roadmap)

H1 (layout engine) — priority 1:
- [x] step 1: spec + presets
- [x] step 2: engine.rs (build_window)
- [x] step 3a: split into build_nav/status/center
- [x] step 3b: wire build_nav into app.rs
- [ ] **step 4a: wire build_status**
- [ ] **step 4b: wire build_center**
- [ ] step 5: remove old panel code, skin.layout_spec
- [ ] step 6: fix MT-2/MT-3/MT-4/UPD-5 (minimap layout, 9-point
  positioning, tooltip overlap, button contrast)
- [ ] step 7: test 800x600 → 4K × 2 skins

After H1:
- [x] Merge UI branch for 0.7.36; publish commit and tag after approval/auth
- [ ] fix AU-2 (version compare, not sha256)
- [ ] fix C1-B (overlay position at start)
- [ ] fix UPD-3/4
- [ ] fix C6a (console in debug), C6b (WinRT notifications)
- [ ] rate limit key pool (TOOL-1)
- [ ] D1 design polish (see docs/design_reference/*.png)

## Known issues (open, current)

- **MT-2**: minimap layout rough (button positioning edge cases)
- **MT-3**: 9-point positioning sometimes broken
- **MT-4**: tooltip contrast on dark theme in some cases
- **UPD-3**: dialog shows X→X when versions match (AU-2 related)
- **UPD-4**: new window after update opens in tray, not foreground
- **UPD-5**: button text contrast on accent (fix in H1 step 6)
- **C1-B**: overlay position at start (LOW)
- **C2**: first-click drag on borderless windows (MEDIUM)
- **C6a**: console window visible in debug builds
- **C6b**: notifications show "Windows PowerShell" instead of app name
- **C4**: main window position not saved
- **AU-2**: plan_update compares sha256, not versions
- **AU-3**: origin/beta is a ghost branch (not read by code)
- **ARC-1**: make-manifest.ps1 -Archive overwrites old archive binary
- **TOOL-1**: rate limit bypass via API key pool (MEDIUM)

Full details in docs/tasks.md.

## Last fixes (sprint 2026-09-30 .. 2026-10-02)

- C5 single-instance
- C8 build.rs (auto BUILD)
- C9 v2 tray show via WinAPI
- C11 ComboBox saves
- C12 disable button
- C14 updater mutex
- C15 restart after update
- AU-1 manual check respects channel
- Merge incident fixed: Tray-session force-pushed after wrong merge

See CHANGELOG.md for full history (English from 0.7.35 onward).

## Key files for next session

- docs/tasks.md — full task list with statuses
- docs/bugs.md — historical fixes
- docs/publish_checklist.md — order of release operations
- CHANGELOG.md — release notes
- src/ui/layout/spec.rs, presets.rs, engine.rs — H1 code (~350 lines)
- src/app.rs:3002-3134 — panels to be replaced by engine

## Rules

- Token economy: don't change model mid-session (breaks prompt cache)
- Subagents for heavy operations (search, tests, logs)
- One task = one commit, atomic
- Push only after user verification
- English for changelog since 0.7.35
- Publish order strict: bump → push ui → merge → tag → release
- Full publish checklist in docs/publish_checklist.md

## How to start (for Opus 5.5)

1. Read docs/HANDOFF.md (this file).
2. Read docs/tasks.md — full queue.
3. Read docs/publish_checklist.md — release rules.
4. Read src/ui/layout/{spec,presets,engine}.rs — H1 code.
5. Confirm understanding: current HEAD, next task (H1 step 4a),
   constraints (one commit, no push, app.rs only for status block).
6. Start H1 step 4a.

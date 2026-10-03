# Changelog

Краткая история релизов. Полная — в приложении («О программе» →
«Журнал изменений»).

## [0.7.36] - 2026-10-03

### Summary
Clearer navigation and settings, plus smoother background refreshes.

### Fixed
- UI scaling now follows each monitor's native DPI
- GPU checks and session totals refresh in the background
- Main window uses the native Windows title bar for move and maximize

### New
- Section tabs, button tips, quick settings, and a persistent status bar

## [0.7.35] - 2026-10-02

### Summary
Under the hood, the update module was rewritten from scratch: the
application now reliably performs updates via manual or silent mode,
with a confirmation dialog and rollback on failure. The UI theme
system (color tokens) has also been refactored.

### Fixed
- Update module: retried download, atomic file replace, rollback on failure
- Restart after update: new process spawns correctly with --updated flag
- Updater mutex race that caused "already running" false errors
- Single-instance: launching twice now focuses the existing window
- Build date: generated automatically via build.rs
- Tray "Show window" via WinAPI (Minimized(true) replaces Visible(false))
- Update channel / frequency ComboBox now saves selection
- Update check now respects the selected channel

### New
- Update confirmation dialog with changelog and countdown (silent mode)
- Localized updater log at logs/updater.log
- Channel-aware manifest fetching

### Changed
- Updater pipeline: mutex -> reexec -> wait_pid -> download -> replace -> spawn
- Theme tokens centralized (ui/theme/tokens.rs)

### Known issues
- Layout engine (H1) pending: some UI blocks may float on resized windows
- Minimap: 9-point positioning edge cases
- Tooltip contrast on dark theme in some cases
- Console window visible in debug builds (hidden in release)

## [0.7.33-beta.2] — 2026-10-01

### Added
- Сторож обновления: rollback quit_requested при падении апдейтера
- Mutex Local\TraySession_Updater против параллельного запуска

### Fixed
- C13a: retry download, обогащённые сообщения об ошибках
- C13b: окно не зависает чёрным после провала апдейтера
- C12: кнопка «Установить сейчас» блокируется
- C14: два апдейтера одновременно
- C6a: консольное окно в release
- C5: single-instance
- C8: BUILD через build.rs
- C9 v2: показать окно из трея через WinAPI
- C11: ComboBox канала/частоты сохраняет выбор
- AU-1: ручная проверка канала

### Known issues
- C9 v3: single ЛКМ по иконке трея не показывает окно (dbl-click работает)
- C1-B: позиция overlay при старте программы
- C2: first-click drag borderless-окна

## [0.7.33-beta.1] — 2026-10-01

Первый pre-release из ui-ветки (ветка app-Tray_session):
UI 1.2.x на theme-токенах, fix(C1) позиции оверлея (частично),
fix(C5) single-instance, fix(AU-1) ручной проверки канала.

## [0.7.32] — 2026-09-29

- Fix: Проводник открывался дважды при удалении; всплывал в тестах.
- New: регрессионные тесты однократного открытия Проводника.

## [0.7.31] — 2026-09-29

- Fix: отменённое обновление уничтожало программу (возврат копии).
- Fix: релиз собирался до бампа версии; проверки версии в .exe и манифеста.
- Fix: нулевое ожидание сигнала, тесты мимо C:, --export-to.

Более ранняя история — только in-app («О программе» → «Журнал изменений»).

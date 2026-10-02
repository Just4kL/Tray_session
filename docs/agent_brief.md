# Agent Brief — Tray Session

> Точка входа для любой AI-сессии. Читать ПЕРВОЙ. Обновлять в конце каждого шага.
> Если противоречит коду — верить коду, но обновить файл.

Последнее обновление: 2026-10-02
Обновил: агент-сессия — updater end-to-end закрыт живым тестом, tasks-догон (ARC-1/MT/UPD-2), бриф — этим коммитом

## Проект

Tray Session — трекер игровых сессий (Windows, трей, будильники, GPU-гейт).
Rust 2021, egui 0.28 + eframe 0.28, SQLite (rusqlite bundled), sysinfo 0.30.13,
nvml-wrapper, winreg/winapi, reqwest, tray-icon, notify-rust.

Репозиторий: `F:\Programms\Tray Session python\game-session-tracker`
Python-версия (pythonProject) — эталон для дымовых тестов, в поставку не входит.

## Текущее состояние

- Ветка: `ui/phase-1-theme`
- HEAD: `aaef02a` (release: 0.7.33-beta.7 binaries + manifest; docs-коммит — HEAD)
- Тесты: 165 зелёных
- Clippy `': error'` = 41 (pre-existing долг проекта, не наш; база была 42)
- Дерево: чистое
- Тег отката: `pre-ui-refactor` (`a48813d`)

## В работе

Пусто: updater end-to-end закрыт живым тестом 2026-10-02 (beta.6+фиксы →
beta.7, ok=true, перезапуск произошёл). Следующее берётся из очереди ниже.

**Побочный эффект фикса RAM (ожидаемый, зафиксирован):** RAM-fallback гейт
стал строже. До фикса он пропускал всё (баг), после — работает как задумано.
Не откатывать.

## Что сделано

| Коммит | Что | Артефакты |
|---|---|---|
| `121cd12` | UI 3.0: TrackerApp → AppState + Views | `src/app_state.rs`, `tools/split-state-3.0.ps1` |
| `c1cf108` | 1.1: theme structs + Skin trait + ThemeManager | `src/ui/theme/{mod,tokens,default,dark}.rs` |
| `01fcf1d` | 1.2.a.1: полная Palette (25 полей) | `src/ui/theme/tokens.rs`, `default.rs` |
| `eac2c58` | 1.2.a.2: `steam_visuals()` → `DefaultSkin::egui_style()` | `src/ui/theme/tests.rs` (golden), `app.rs` |
| `b6916ec` | 1.2.a.3: три функции на токенах, `UPDATE_YELLOW` удалён | `app.rs`, `tools/adapt-tests-1.2a.3.ps1` |
| `42365fd` | fix RAM: `memory()/1024` → `bytes_to_mb` в `monitor.rs:94`, `detector.rs:412` | `src/gpu.rs` (хелпер + тест), `docs/bugs.md` |
| `b4e668a` | 1.2.a.4: +5 токенов, свап 20 мест (B: 9, Q: 11), бриф → `docs/` | `src/app.rs`, `src/ui/theme/*`, `docs/*`, `tools/adapt-calls-1.2a.4.ps1`, `fix-skin-1.2a.4.ps1` |
| `2feb126` | AU-1: ручная проверка учитывает канал + AUTO-UPD в tasks.md | `src/app.rs`, `src/update.rs` (allow), `docs/tasks.md`, `docs/agent_brief.md` |
| `785705f` | fix(C5): single-instance через именованный mutex | `src/main.rs`, `Cargo.toml` (фичи winapi) |
| `a598516` | chore: bump version to 0.7.33-beta.1 | `Cargo.toml`, `Cargo.lock`, `src/app.rs` |
| `f127ffb` | release: 0.7.33-beta.1 binaries + manifest | `TraySession.exe`, `Tray_session_setup.exe`, `update_manifest.json` |
| `2467fdc` | docs: agent_brief под 0.7.33-beta.1 | `docs/agent_brief.md` |
| `67101e5` | fix: ComboBox-черновики (C10/C11), show_main_window WinAPI (C9), BUILD из build.rs (C8) | `src/app.rs`, `src/app_state.rs`, `build.rs`, `Cargo.toml` |
| `04f78cc` | fix(C9): скрытие в трей через Minimized + guard-тест | `src/app.rs` |
| `9646921` | fix(C9 v2): показ окна из трея через WinAPI + taskbar(false) | `src/main.rs`, `src/app.rs` (hide_tray) |
| `071da14` | chore: bump version to 0.7.33-beta.2 | `Cargo.toml`, `Cargo.lock`, `src/app.rs` |
| `4efcac4` | fix(C6a): скрыть консольное окно в release-сборке | `src/main.rs` |
| `bdb4760` | release: 0.7.33-beta.2 binaries + manifest | `TraySession.exe`, `Tray_session_setup.exe`, `update_manifest.json` |
| `410bb6c` | docs(DOC-1): CHANGELOG.md + запись 0.7.33-beta.2 в app.rs | `src/app.rs`, `CHANGELOG.md`, `BRANCHING.md` |
| `f60c8d2` | docs(LIC-1): аудит лицензий под Proprietary | `docs/licenses.md`, `THIRD_PARTY_LICENSES.md`, `tools/make-licenses.ps1` |
| `4f1b389` | docs(DOC-2/3/4): Proprietary LICENSE, TERMS.md, авторство | `LICENSE`, `TERMS.md`, `src/app.rs`, `Cargo.toml` |
| `9c009c8` | fix(C13c): watchdog по updater.lock вместо таймаута 90 с | `src/update.rs` (lock + guard), `src/app.rs` (сторож), `src/main.rs` (stale-cleanup), `src/update/tests.rs` (budget-тест) |
| `86d0582` | chore: bump version to 0.7.33-beta.3 | `Cargo.toml`, `Cargo.lock`, `src/app.rs` |
| `dad3c64` | release: 0.7.33-beta.3 binaries + manifest | `TraySession.exe`, `update_manifest.json` (installer без изменений) |
| `96e3599` | docs: agent_brief под 0.7.33-beta.3 | `docs/agent_brief.md` |
| `fcf8650` | docs: ARC-1 — make-manifest -Archive архивирует не ту сборку | `docs/tasks.md` (только запись задачи, без фикса) |
| live-test | beta.2 → beta.3: ok=true, exe заменён, lock снят; перезапуск НЕ сработал → C15 | `update_report.json`, `docs/bugs.md` (C15), `docs/tasks.md` (C15/C16/AU-4/SUI-1) |
| `847166e` | fix(C15): main_pid.txt, wait_for_main_pid_death, самозакрытие по ok=true | `src/main.rs`, `src/update.rs`, `src/app.rs`, `Cargo.toml` (фичи winapi) |
| `9e27d3a` | chore: bump to 0.7.33-beta.4 | `Cargo.toml`, `Cargo.lock`, `src/app.rs` |
| `6bcd2a3` | release: 0.7.33-beta.4 binaries + manifest | `TraySession.exe`, `update_manifest.json` (installer без изменений) |
| `47991c2` | refactor(update): updater как единый pipeline (ulog, reexec, atomic replace, wait_pid, --updated) | `src/update.rs`, `src/app.rs`, `src/app_state.rs`, `src/main.rs`, `src/update/tests.rs` (160 тестов) |
| `0d6af4f` | chore: bump to 0.7.33-beta.5 + TOOL-1 | `Cargo.toml`, `Cargo.lock`, `src/app.rs`, `docs/tasks.md` |
| `348662e` | release: 0.7.33-beta.5 binaries + manifest | `TraySession.exe`, `update_manifest.json` (installer без изменений) |
| `f420aa3` | fix(update): явный --base сквозь pipeline (reexec ломал program_dir) | `src/update.rs`, `src/update/tests.rs`, `src/main.rs` |
| `110251d` | chore: bump to 0.7.33-beta.6 | `Cargo.toml`, `Cargo.lock`, `src/app.rs` |
| `4af3a80` | release: 0.7.33-beta.6 binaries + manifest | `TraySession.exe`, `update_manifest.json` (installer без изменений) |
| `2fba0a1` | fix(update): mutex lifecycle — RAII guard, owner-PID stale-detect, диагностика | `src/update.rs`, `src/update/tests.rs` |
| `c268a14` | chore: bump to 0.7.33-beta.7 | `Cargo.toml`, `Cargo.lock`, `src/app.rs` |
| `aaef02a` | release: 0.7.33-beta.7 binaries + manifest | `TraySession.exe`, `update_manifest.json` (installer без изменений) |
| `b3a0507..e8e3211` | updater end-to-end: живой тест успешен 2026-10-02, beta.7 | report ok=true, перезапуск произошёл, `logs/updater.log` пошагово |
| `3fb82c5` | docs: ARC-1 обновлён, MT-1/MT-2/MT-3, UPD-2 в tasks | `docs/tasks.md` |
Детали RAM-фикса (факт, не план): хелпер `bytes_to_mb` добавлен в `gpu.rs`,
`vram_mb` **не тронута побайтово** (делегирования нет — было ограничение
«не менять формулу»). Тест — `gpu::tests::memory_bytes_to_mb_is_1024_based`
(тест лежит рядом с хелпером, а не в `monitor::tests`). Живой старт
debug-сборки: 12 секунд полёт нормальный; число RAM в статус-баре
прочитать headless невозможно.

## Очередь следующего

Единый бэклог: `docs/tasks.md` (чек-лист со статусами `[ ]/[~]/[x]`, датами,
приоритетами; закрытые — в архиве внизу файла). Полная очередь — там.

1. **MT-1** — кнопки мини-трея не работают (HIGH). Следующая задача.
2. **Фаза 2** — `LayoutSpec` + `engine.rs` (H1, включает MT-2/MT-3). Лечит layout со скриншота:
   панели не привязаны к `screen_rect`, блоки плывут.
2. **Фаза 3** — вынос `ui_*` в `views/*` (L2).
   Порядок: about → shortcuts → timer → alarms → games → sessions.
3. **Фаза 4** — реестр действий для кнопок (M3).
4. **TODO:** нативные уведомления через WinRT (L1; сейчас fallback на PowerShell,
   заголовок «Windows PowerShell», нет иконки).

## Ключевые решения (не переобсуждать)

- Цвета — только через `tokens.palette.*`. Никаких `Color32::from_*` в виджетах.
- При сомнении в маппинге цвета заводить именованный токен, не сваливать
  в «почти совпадает» (прецедент 1.2.a.4: `attention`/`recording`/`heading`/
  `subtle`/`dim` вместо сведения к ближайшему).
- Скины data-driven. Новые поля в `Palette` добавляются под конкретное
  назначение, не «на будущее».
- Layout — только через `engine.rs`. Виджеты не знают про `ctx` и панели.
- Тема применяется через `ctx.set_visuals` (**не** `set_style` — тот сбрасывает
  spacing). Переход на полный `set_style` — фаза 1.4.
- Golden-тест `src/ui/theme/tests.rs` — эталон Visuals от `121cd12`. Менять
  только при осознанной смене темы, с явным указанием полей в коммите.
- Ветка ведётся от `121cd12`. Пуш — только по явному добру после приёмки.
- Тег ставится ТОЛЬКО после коммита и только на закоммиченный HEAD.
  Порядок: коммит → status пусто → тег → push ветки → push тега.
  Инцидент 2026-10-01: тег дважды уходил на устаревший хеш.
- Amend только до пуша. Если коммит уже запушен или тегирован — не
  амендить, обновления идут следующим коммитом.

## Известные ловушки (не регрессии)

- Clippy `': error'` = 41 — pre-existing долг. Новые не добавляем.
- Методика счёта clippy: паттерн `': error'` (short-формат), НЕ `'error\['`.
- Rustfmt reflow даёт 100+ «удалённых» строк при чисто префиксной замене.
  Проверяется счётчиками `let/if/match/return/for/while/fn` до/после.
- `DefaultSkin::default()` пересобирает `Tokens` на каждый вызов.
  В тестах — `let skin = DefaultSkin::default();` в начале функции.
- `tools/*.ps1` — one-shot скрипты, идемпотентны. Повторный прогон не нужен.
- sysinfo 0.30: `Process::memory()` = **байты** (до 0.30 — КБ).
  Док на `gpu::bytes_to_mb`, тест `memory_bytes_to_mb_is_1024_based`,
  заметка в `docs/bugs.md`.
- Layout сломан (панели плывут) — pre-existing, лечится в фазе 2.
- «Установить сейчас» до рерайта updater висло (C13a), теряло exe (C13b),
  не перезапускалось (C15). Закрыто живым тестом 2026-10-02. Если
  симптомы вернутся — taskkill `_updater_running.exe`, смотри
  `logs/updater.log` и `update_tmp/update_report.json` (stage/error_kind).
- В детач-HEAD можно случайно закоммитить мимо ветки. `git status` перед коммитом.
- В `app.rs` есть растеризаторы часов и превью — их цвета **не подчиняются теме**,
  они в audit-исключениях.
- Неприменённый stash с бинарниками сборок (`TraySession.exe`,
  `Tray_session_setup.exe`, исходников нет) — оставлен как был, не дропать
  без разбора.
- ARC-1: `make-manifest.ps1 -Archive` перезаписывает
  `dist/old/<old-name>.exe` НОВЫМ бинарником. Все
  `dist/old/TraySession_0.7.33-beta.N.exe` (N=1..6) — stale/wrong.
  Восстановление настоящего старого: `cmd /c "git show
  <prev-tag>:TraySession.exe > dist/old/TraySession_X_REAL.exe"`.

## Артефакты в репо

- `docs/ui_refactor_audit.md` — карта фаз, объёмы, оставшиеся `allow`, roadmap
- `docs/tasks.md` — единый бэклог (CRITICAL/HIGH/MEDIUM/LOW/DESIGN),
  раздел AUTO-UPD — зафиксированная модель авто-обновления (AU-1 закрыт)
- `docs/design_reference/` — PNG-референсы ComfyUI для D1 (проверены: 4 файла,
  валидный PNG; `comfyui-topbar.png` просмотрен — тёмная карточка, пилюля
  бренда + Update, диалог Cancel/Update Now)
- `docs/bugs.md` — ловушки единиц измерения (sysinfo units)
- `src/ui/theme/tests.rs` — golden-тест Visuals
- `tools/split-state-3.0.ps1` — one-shot, выполнен в `121cd12`
- `tools/adapt-tests-1.2a.3.ps1` — one-shot, выполнен в `b6916ec`

## Формат отчёта агента

1. `git --no-pager diff <base> HEAD -- <files>` (не только `--stat`)
2. `git --no-pager diff --stat <base> HEAD`
3. `cargo test` — итоговая строка
4. clippy `': error'` count
5. Точки разведки **до** правки
6. Коммит-мессадж (готовый к копипасту)
7. Отклонения от задания — явным списком, с обоснованием

## Правила коммуникации с агентом

- Скоуп фиксируется явно (что **НЕ** трогать) — иначе агент расширит.
- «Не закоммитил, жду визуальной проверки» — валидный стоп-стейт. Не давить.
- Отклонения приветствуются, если обоснованы и сужают диф.
- Тесты не править под код — править код под тесты.
- Визуальную проверку агент сделать не может (нет GUI). Приёмка визуала — за
  человеком. Golden-тесты — приемлемая замена для цветов, но не для layout.

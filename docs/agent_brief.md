# Agent Brief — Tray Session

> Точка входа для любой AI-сессии. Читать ПЕРВОЙ. Обновлять в конце каждого шага.
> Если противоречит коду — верить коду, но обновить файл.

Последнее обновление: 2026-10-01
Обновил: агент-сессия — бэклог `docs/tasks.md` + синхронизация брифа

## Проект

Tray Session — трекер игровых сессий (Windows, трей, будильники, GPU-гейт).
Rust 2021, egui 0.28 + eframe 0.28, SQLite (rusqlite bundled), sysinfo 0.30.13,
nvml-wrapper, winreg/winapi, reqwest, tray-icon, notify-rust.

Репозиторий: `F:\Programms\Tray Session python\game-session-tracker`
Python-версия (pythonProject) — эталон для дымовых тестов, в поставку не входит.

## Текущее состояние

- Ветка: `ui/phase-1-theme`
- HEAD: `b4e668a` (UI 1.2.a.4: остаточные хардкоды → токены; +5 токенов; бриф в docs/)
- Тесты: 157 зелёных
- Clippy `': error'` = 41 (pre-existing долг проекта, не наш; база была 42)
- Дерево: чистое
- Тег отката: `pre-ui-refactor` (`a48813d`)

## В работе

Ничего. Последний шаг (1.2.a.4) закоммичен (`b4e668a`), дерево чистое.

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

Детали RAM-фикса (факт, не план): хелпер `bytes_to_mb` добавлен в `gpu.rs`,
`vram_mb` **не тронута побайтово** (делегирования нет — было ограничение
«не менять формулу»). Тест — `gpu::tests::memory_bytes_to_mb_is_1024_based`
(тест лежит рядом с хелпером, а не в `monitor::tests`). Живой старт
debug-сборки: 12 секунд полёт нормальный; число RAM в статус-баре
прочитать headless невозможно.

## Очередь следующего

Единый бэклог: `docs/tasks.md`.

1. **Фаза 2** — `LayoutSpec` + `engine.rs` (H1). Лечит layout со скриншота:
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
- В детач-HEAD можно случайно закоммитить мимо ветки. `git status` перед коммитом.
- В `app.rs` есть растеризаторы часов и превью — их цвета **не подчиняются теме**,
  они в audit-исключениях.
- Неприменённый stash с бинарниками сборок (`TraySession.exe`,
  `Tray_session_setup.exe`, исходников нет) — оставлен как был, не дропать
  без разбора.

## Артефакты в репо

- `docs/ui_refactor_audit.md` — карта фаз, объёмы, оставшиеся `allow`, roadmap
- `docs/tasks.md` — единый бэклог (CRITICAL/HIGH/MEDIUM/LOW/DESIGN)
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

# Аудит presentation-слоя перед модульным UI

> **Status: ARCHIVED (2026-10-02).** Historical audit of UI refactor
> phases 1.0-1.2. For current state see docs/HANDOFF.md and docs/tasks.md.

Снято на этапе 3.0 (коммит расщепления `TrackerApp` → `AppState`).
Источник — grep по `src/app.rs`. Цель — точный объём для фаз 1, 2, 5.

## Масштаб

| Метрика | Значение |
|---|---|
| Обращений `self.` до сплита | 719 |
| Уникальных имён `self.X` | 165 |
| Полей в `TrackerApp` (все ушли в `AppState`) | 100 |
| Замен `self.F` → `self.state.F` скриптом | 575 |
| Коллизий «поле + метод с одним именем» | 1 (`stopwatch_start`, разведено руками) |
| Многострочных цепочек `self\n.field` (регексп не взял) | 5, исправлены руками |

## Цвета (фаза 1: всё в `tokens.palette`)

| Паттерн | Вхождений |
|---|---|
| `Color32::from_rgb / from_gray / WHITE / BLACK / LIGHT_* / DARK_* / YELLOW / GREEN / RED / BLUE / GOLD` | 65 |
| `RichText...color(` | 11 |
| `.fill(` | 7 |
| `Stroke::new` | 31 |

## Геометрия (фаза 1: всё в `tokens.spacing`, фаза 2: в `LayoutSpec`)

| Паттерн | Вхождений |
|---|---|
| `ui.add_space(N)` с числовым литералом | 16 |
| `SidePanel::left("nav").exact_width(88.0)` | 1 (`app.rs`, строка ~2810) |
| `TopBottomPanel::bottom("status").exact_height(24.0)` | 1 (строка ~2862) |

## Панели (фаза 2: только `ui/layout/engine.rs`)

Боевые (в `update()`):
- `SidePanel::left("nav")` — строка ~2808
- `TopBottomPanel::bottom("status")` — строка ~2860
- `CentralPanel::default().show(ctx, ...)` — строка ~2882

Остальные `CentralPanel` — тесты/превью (`nav_icons_preview_png`,
`ui_preview_png`, оверлеи, строки ~2101, 2335, 6266, 6355, 6750, 6821,
6915, 7358). Их не трогаем: это не каркас окна, а изолированные
поверхности для растеризатора.

## Кнопки (фаза 4: реестр действий)

| Паттерн | Вхождений |
|---|---|
| `Button::new` / `ui.button(` / `.small_button(` | 65 |

Первый потребитель реестра — кнопка обновления в правом верхнем углу
`CentralPanel` (сейчас захардкожена).

## Чистота слоёв (фаза 5: должно остаться так)

Прямых упоминаний бэкенда в `app.rs` — ноль в коде: `sysinfo`,
`nvml`, `rusqlite`, `reqwest` встречаются только в строках changelog,
подписях и комментариях (7 вхождений, все текстовые). Весь доступ к
железу/БД/сети идёт через модули `monitor`, `db`, `update`, `gpu`,
`detector` — представление их типов не касается.

## Разделы (фаза 3: порядок выноса от простого к сложному)

| Вью | Метод | Состояние (уже сгруппировано в `AppState`) |
|---|---|---|
| about | `ui_about` | `about_sub` — пилот, почти без состояния |
| shortcuts | `ui_shortcuts` | `shortcuts`, `capture_action`, `capture_armed_at` |
| timer | `ui_timer` | `timer_*` (8 полей) |
| alarms | `ui_alarms` | `alarm_*`, `alarms`, `edit_alarm`, `show_analog`, `analog_alarm_id` |
| games | `ui_games` | `search`, `api_key`, `steam_id`, `scan_msg`, `show_picker`, `picker_*`, `show_folder`, `folder_*` |
| sessions | `ui_sessions` | `agg`, `show_*_col`, `detail_game`, `edit_session` — самый большой, последним |

Отдельно, после вкладок: `windows()` → `ui/views/dialogs.rs`,
`show_stopwatch_overlay` / `show_session_strip` → `ui/overlays.rs`.

## Оставшийся allow(dead_code) в src/ui/theme/ (обновлено на 1.2.a.4)

Фактический список (`Select-String -Path src/ui/theme/*.rs -Pattern
'allow\(dead_code\)'`). Правило: allow не переставляется со структур на
поля, пока clippy реально не падает, — структурный allow покрывает
непрочитанные поля целиком. Исключение — поля `err`/`default`: их
структуры в основном читаются, поэтому allow висит точечно на полях.

| Файл:строка | Что под allow | Когда снимем |
|---|---|---|
| tokens.rs `Spacing` | все поля (xs/sm/md/lg/nav_w/status_h/row_h) | фаза 2 — при замене геометрии в каркасе |
| tokens.rs `Radii` | `panel` (`widget` читается в `egui_style`) | фаза 2 — при замене скруглений |
| tokens.rs `Typography` | все поля (body/caption/heading) | фаза 2 — при маппинге шрифтов в `egui_style` |
| tokens.rs `BorderPalette.default` | поле (остальные читаются) | фаза 2 — потребитель default пока не переведён |
| tokens.rs `SemanticPalette.err` | поле (остальные читаются) | фаза 2 — потребитель err пока не переведён |
| tokens.rs `Tokens` | поля `spacing`, `typography` (остальные читаются) | вместе с `Spacing`/`Typography` |
| mod.rs `trait Skin` | методы `id()`, `tokens()` (`egui_style()` читается) | 1.4 — `id()` для переключателя, `tokens()` для прямого чтения |
| mod.rs `ThemeManager::set` | весь метод | 1.4 — переключатель скинов в «О программе» |
| dark.rs `DarkSkin` | структура целиком | 1.4 — реальная инверсия цветов |

Сняты в 1.2.a.2: `Strokes`, `InteractiveState`, `InteractivePalette`,
`AccentPalette` (все поля читаются в `egui_style`), `DefaultSkin.tokens`,
`ThemeManager` (структура), `ThemeManager::current`.

Сняты в 1.2.a.4: `BgPalette` (все 7 полей читаются, включая `raised`
в рамке полоски), `TextPalette` (все 4, включая `heading` заголовков),
`Palette` (все 6). Проверено grep-ом: `semantic.ok` читается в
`strip_row`, `semantic.warn` — в `ui_games`.

## Категория B + Q закрыта (1.2.a.4)

Свап 20 мест: B — 9 (`manual_section`, оверлеи, `ui_games`, `page_title`,
`card`, `strip_row`), Q — 11 (+5 новых токенов `semantic.attention`/
`recording`, `text.heading`, `border.subtle`, `accent.dim`). Остаток
хардкодов в production: только растеризатор часов (audit-исключения),
`from_rgba_unmultiplied`-композиции, `blend()`, `TRANSPARENT`.

## Roadmap: ctx.set_visuals → ctx.set_style

`app.rs:2761`: сейчас `ctx.set_visuals(theme.current().egui_style().visuals)`,
потому что `egui_style()` возвращает только `Visuals` (spacing/typography
из токенов пока не применяются). В фазе 1.4 при добавлении dark-темы —
перейти на полный `ctx.set_style(...)` одним коммитом, проверив, что
`ui.spacing_mut(...)` в `app.rs` не сломались.
`ThemeManager::set()` (`mod.rs`) использует `ctx.set_style` — при 1.4
пересмотреть согласованно с `app.rs:2761`.
`app.rs:6302` и `6768` (тесты) — форма `ctx.set_style(egui::Style {
visuals, ..Default })` оставлена намеренно, менять не нужно.

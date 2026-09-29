# Аудит presentation-слоя перед модульным UI

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

## Оставшийся allow(dead_code) в src/ui/theme/ (снято на 1.2.a.2)

Фактический список (`Select-String -Path src/ui/theme/*.rs -Pattern
'allow\(dead_code\)'`). Правило: allow не переставляется со структур на
поля, пока clippy реально не падает, — структурный allow покрывает
непрочитанные поля целиком.

| Файл:строка | Что под allow | Когда снимем |
|---|---|---|
| tokens.rs:10 `Spacing` | все поля (xs/sm/md/lg/nav_w/status_h/row_h) | 1.2.a.3 — при замене геометрии в каркасе |
| tokens.rs:17 `Radii` | `panel` (`widget` читается в `egui_style`) | 1.2.a.3 — при замене скруглений |
| tokens.rs:30 `Typography` | все поля (body/caption/heading) | фаза 2 — при маппинге шрифтов в `egui_style` |
| tokens.rs:34 `BgPalette` | `raised` (остальные 6 читаются) | 1.2.a.3 — если найдётся потребитель, иначе фаза 2 |
| tokens.rs:63 `TextPalette` | `muted` (`primary`, `on_accent` читаются) | 1.2.a.3 — при замене приглушённого текста |
| tokens.rs:71 `BorderPalette` | `default` (`interactive` читается) | 1.2.a.3 — при замене рамок |
| tokens.rs:78 `SemanticPalette` | все поля (ok/warn/warn_soft/err) | фаза 2 — семантика красится напрямую в виджетах, не через `Visuals` |
| tokens.rs:87 `Palette` | поле `semantic` (остальные 5 читаются) | вместе с `SemanticPalette`, фаза 2 |
| tokens.rs:98 `Tokens` | поля `spacing`, `typography` (остальные читаются) | вместе с `Spacing`/`Typography` |
| mod.rs:19 `trait Skin` | методы `id()`, `tokens()` (`egui_style()` читается) | 1.4 — `id()` для переключателя, `tokens()` для прямого чтения |
| mod.rs:49 `ThemeManager::set` | весь метод | 1.4 — переключатель скинов в «О программе» |
| dark.rs:11 `DarkSkin` | структура целиком | 1.4 — реальная инверсия цветов |

Сняты в 1.2.a.2 (больше не нужны): `Strokes`, `InteractiveState`,
`InteractivePalette`, `AccentPalette` (все поля читаются в `egui_style`),
`DefaultSkin.tokens`, `ThemeManager` (структура), `ThemeManager::current`.

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

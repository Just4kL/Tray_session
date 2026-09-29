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

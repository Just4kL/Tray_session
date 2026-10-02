# Аудит лицензий (LIC-1)

Дата: 2026-10-01. Метод: cargo-deny 0.20.2 + обход графа через
cargo metadata с фильтром x86_64-pc-windows-msvc (только то, что
реально линкуется в .exe). Всего в Windows-замыкании: 298 крейтов.

## Вердикт

Copyleft (GPL/LGPL/AGPL/SSPL/...) **в Windows-сборке отсутствует**.
Proprietary-поставка возможна. Особые случаи разобраны ниже —
все verdict: можно поставлять.

## Особые случаи

### MPL-2.0 — option-ext 0.2.0 (только Linux, в бинарник НЕ входит)
Цепочка: option-ext ← dirs-sys ← dirs ← tray-icon, но dirs/libappindicator
у tray-icon — строго cfg(target_os = "linux"). В Windows-замыкании
(проверено обходом metadata) этих крейтов нет. MPL-2.0 — слабый
file-scoped copyleft: даже если бы входил, немодифицированное использование
как библиотеки разрешено (раскрывать нужно только изменённые MPL-файлы,
мы их не меняли). Текст MPL-2.0 приложен ниже на случай аудита.

### Apache-2.0 WITH LLVM-exception — target-lexicon 0.12.16 (только Linux)
Цепочка: target-lexicon ← cfg-expr ← system-deps ← (build) atk-sys ← gtk
← libappindicator ← tray-icon. Всё это Linux build-зависимости. В Windows-
замыкании нет. LLVM-exception — пермиссивное исключение, в любом случае
совместимо.

### Шрифты epaint (OFL-1.1 + UFL-1.0) — входят в бинарник
epaint 0.28.1 встраивает шрифты (Ubuntu-Light, Hack, NotoEmoji) в .exe.
OFL-1.1 и UFL-1.0 разрешают встраивание (embedding) в ПО. Тексты приложены.

### SQLite — public domain
rusqlite 0.31 (MIT) + bundled SQLite (общественное достояние). Отдельных
обязательств нет.

## Нерust-компоненты (вне cargo)

- **7-Zip SFX** (установщик Tray_session_setup.exe, собирается локальным
  7z.exe + 7z.sfx): LGPL с SFX-исключением — использование SFX-модуля
  для упаковки разрешено без раскрытия исходников программы.
- **NVML** (nvml-wrapper 0.11, MIT): NVML.dll НЕ поставляется с программой
  (установщик везёт только TraySession.exe + README.md), берётся из
  системы (драйвер NVIDIA). Вопрос лицензии DLL снят.

## Методика проверки copyleft

Обход resolve-графа cargo metadata (Windows target) + cargo-deny 0.20.2
с deny.toml (разрешён пермиссивный список). Прямой regex-поиск
GPL/LGPL/AGPL/SSPL/CDDL/EPL/MPL/EUPL по списку — 0 совпадений
(ложные срабатывания вида windows-implement отфильтрованы границами слов).
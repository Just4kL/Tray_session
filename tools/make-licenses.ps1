# LIC-1: assemble docs/licenses.md + THIRD_PARTY_LICENSES.md
#
# Inputs (already gathered):
#   target/deplist.txt      - "name version | license" for the Windows closure
#   local cargo registry    - verbatim license texts (no retyping)
# Run from project root.

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$reg = "$env:USERPROFILE\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f"

function Get-Text($rel) {
    $p = Join-Path $reg $rel
    if (-not (Test-Path $p)) { throw "missing: $rel" }
    $t = [System.IO.File]::ReadAllText($p, [System.Text.UTF8Encoding]::new($false))
    # Normalize CRLF for stable diffs.
    return ($t -replace "`r`n", "`n").TrimEnd()
}

$mit     = Get-Text 'adler2-2.0.1\LICENSE-MIT'
$apache  = Get-Text 'adler2-2.0.1\LICENSE-APACHE'
$isc     = Get-Text 'rustls-0.23.41\LICENSE-ISC'
$bsd3    = Get-Text 'num_enum-0.7.6\LICENSE-BSD'
$bsd2    = Get-Text 'zerocopy-0.8.59\LICENSE-BSD'
$zlib    = Get-Text 'bytemuck-1.25.2\LICENSE-ZLIB'
$bsl     = Get-Text 'ryu-1.0.23\LICENSE-BOOST'
$unlic   = Get-Text 'aho-corasick-1.1.5\UNLICENSE'
$zeroBSD  = Get-Text 'adler2-2.0.1\LICENSE-0BSD'
$oFL     = Get-Text 'epaint-0.28.1\fonts\OFL.txt'
$ufl     = Get-Text 'epaint-0.28.1\fonts\UFL.txt'
$unicode = Get-Text 'icu_collections-2.2.0\LICENSE'
$mpl     = Get-Text 'option-ext-0.2.0\LICENSE.txt'

$table = (Get-Content 'target\deplist.txt' -Encoding UTF8 | ForEach-Object { "| $($_.Replace(' | ', ' | ')) |" }) -join "`n"
$count = (Get-Content 'target\deplist.txt' -Encoding UTF8 | Measure-Object).Count

$licenses = @"
# Аудит лицензий (LIC-1)

Дата: 2026-10-01. Метод: cargo-deny 0.20.2 + обход графа через
cargo metadata с фильтром x86_64-pc-windows-msvc (только то, что
реально линкуется в .exe). Всего в Windows-замыкании: $count крейтов.

## Вердикт

Copyleft (GPL/LGPL/AGPL/SSPL/...) **в Windows-сборке отсутствует**.
Proprietary-поставка возможна. Особые случаи разобраны ниже —
все verdict: можно поставлять.

## Особые случаи

### MPL-2.0 — option-ext 0.2.0 (только Linux, в бинарник НЕ входит)
Цепочка: option-ext ← dirs-sys ← dirs ← tray-icon, но dirs/libappindicator
у tray-icon — строго `cfg(target_os = "linux")`. В Windows-замыкании
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
"@

[System.IO.File]::WriteAllText(
    'docs\licenses.md', $licenses,
    (New-Object System.Text.UTF8Encoding($false)))

$third = @"
# THIRD_PARTY_LICENSES

Сторонние компоненты в составе TraySession.exe (Windows-сборка).
Полные тексты лицензий — дословные копии из пакетов crates.io.

## Состав ($count крейтов, Windows-замыкание)

| Крейт | Версия | Лицензия |
|---|---|---|
$table

## Тексты лицензий

### MIT

$mit

### Apache-2.0

$apache

### ISC

$isc

### BSD-3-Clause

$bsd3

### BSD-2-Clause

$bsd2

### Zlib

$zlib

### Boost Software License 1.0 (BSL-1.0)

$bsl

### The Unlicense (public domain)

$unlic

### 0BSD

$zeroBSD

### Unicode License (данные ICU)

$unicode

### SIL Open Font License 1.1 (шрифты epaint)

$oFL

### Ubuntu Font Licence 1.0 (шрифт Ubuntu-Light в epaint)

$ufl

### Mozilla Public License 2.0 (только Linux-цепочка, в .exe не входит)

Приложена для полноты аудита: option-ext 0.2.0 (MPL-2.0) тянется только
через Linux-зависимости tray-icon и в Windows-сборку не линкуется.

$mpl
"@

[System.IO.File]::WriteAllLines(
    'THIRD_PARTY_LICENSES.md', ($third -split "`n"),
    (New-Object System.Text.UTF8Encoding($false)))
Write-Host 'docs/licenses.md + THIRD_PARTY_LICENSES.md written'
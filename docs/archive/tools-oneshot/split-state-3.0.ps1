# Этап 3.0 модульного UI: расщепление TrackerApp -> AppState + Views.
#
# Что делает:
#   1. Заменяет определение `pub struct TrackerApp { ... }` на новую
#      оболочку { state, views, theme, last_skin_id }.
#   2. Оборачивает литерал конструктора в `AppState { ... }` (+4 пробела
#      отступа внутри).
#   3. Заменяет `self.FIELD` -> `self.state.FIELD` для всех полей из
#      whitelist (имена берутся из УДАЛЁННОГО определения структуры,
#      руками список не пишется и протухнуть не может).
#
# Безопасность (требования из ревью):
#   - матчит строго `self.FIELD` с word boundary: определение структуры,
#     `Self { ... }`, `self.state.*`, `self.views`, `self.theme` не задеваются;
#   - `self.ui_*` (методы) не в whitelist: он строится только из имён полей;
#   - пересечение имён полей и методов проверяется, о коллизиях — предупреждение;
#   - идемпотентен: повторный прогон ничего не меняет (`state`/`views`/
#     `theme`/`last_skin_id` в whitelist не входят).
#
# Запуск из корня проекта:
#     powershell -ExecutionPolicy Bypass -File tools\split-state-3.0.ps1

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$path = 'src\app.rs'
$lines = [System.IO.File]::ReadAllLines($path, [System.Text.UTF8Encoding]::new($false))

# --- 1. Найти определение структуры -------------------------------------
$start = -1
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -eq 'pub struct TrackerApp {') { $start = $i; break }
}
if ($start -lt 0) { throw 'Не найдена строка `pub struct TrackerApp {`' }
$end = -1
for ($i = $start + 1; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -eq '}') { $end = $i; break }
}
if ($end -lt 0) { throw 'Не найден конец структуры TrackerApp' }
if ($lines[$end + 2] -notmatch '^impl TrackerApp') {
    throw "После структуры нет `impl TrackerApp` (строка $($end + 3)): $($lines[$end + 2])"
}

# --- 2. Whitelist полей из удаляемого определения ------------------------
$fields = @()
for ($i = $start + 1; $i -lt $end; $i++) {
    if ($lines[$i] -match '^\s+(?:pub\s+)?([a-z_][a-z0-9_]*)\s*:') {
        $fields += $Matches[1]
    }
}
$fields = @($fields | Sort-Object -Unique)
Write-Host "Полей в whitelist: $($fields.Count)"

# Коллизии с методами: такое имя заменять нельзя (метод ловить не должны,
# но если поле и метод называются одинаково — разбирать руками).
$methods = @()
foreach ($l in $lines) {
    if ($l -match '^\s+(?:pub\s+)?fn\s+([a-z_][a-z0-9_]*)\s*\(') {
        $methods += $Matches[1]
    }
}
$methods = @($methods | Sort-Object -Unique)
$collisions = @($fields | Where-Object { $methods -contains $_ })
if ($collisions.Count -gt 0) {
    Write-Warning ($collisions -join ', ')
    Write-Warning 'Коллизии имён полей и методов — эти имена исключены из замены, проверить руками.'
    $fields = @($fields | Where-Object { $collisions -notcontains $_ })
}

# --- 3. Новое определение оболочки ---------------------------------------
$newStruct = @(
    '/// Приложение: тонкая оболочка eframe поверх состояния и представлений.',
    '///',
    '/// Этап 3.0: бизнес-состояние целиком лежит в `state: AppState`',
    '/// (модуль `app_state`), представления — в `views: Views`, тема — в',
    '/// `theme: ThemeManager` (оба пока пустые, наполняются в фазах 3 и 1).',
    '/// Методы `impl TrackerApp` обращаются к полям через `self.state.*`:',
    '/// замена механическая, выполнена скриптом `tools/split-state-3.0.ps1`.',
    'pub struct TrackerApp {',
    '    pub state: AppState,',
    '    pub views: Views,',
    '    pub theme: ThemeManager,',
    '    /// Id текущего скина: `ctx.set_style` вызывается только при смене,',
    '    /// а не каждый кадр (иначе ломаются анимации).',
    '    last_skin_id: &''static str,',
    '}'
)

$out = New-Object System.Collections.Generic.List[string]
for ($i = 0; $i -lt $start; $i++) { $out.Add($lines[$i]) }
foreach ($l in $newStruct) { $out.Add($l) }
for ($i = $end + 1; $i -lt $lines.Count; $i++) { $out.Add($lines[$i]) }
$lines = $out.ToArray()

# --- 4. Конструктор: обернуть литерал в AppState --------------------------
$ci = [Array]::IndexOf($lines, '        let mut app = Self {')
if ($ci -lt 0) { throw 'Не найден `let mut app = Self {` в конструкторе' }
$out = New-Object System.Collections.Generic.List[string]
for ($i = 0; $i -lt $ci; $i++) { $out.Add($lines[$i]) }
$out.Add('        let mut app = Self {')
$out.Add('            state: AppState {')
# Конец литерала: `last_saved: cfg_handle.clone(),` + закрывающая `};`
$li = -1
for ($i = $ci + 1; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^\s+last_saved: cfg_handle\.clone\(\),$') { $li = $i; break }
}
if ($li -lt 0) { throw 'Не найден `last_saved: cfg_handle.clone(),`' }
if ($lines[$li + 1] -ne '        };') { throw "После last_saved нет `        };`: $($lines[$li + 1])" }
for ($i = $ci + 1; $i -le $li; $i++) { $out.Add('    ' + $lines[$i]) }
$out.Add('            },')
$out.Add('            views: Views::default(),')
$out.Add('            theme: ThemeManager::default(),')
$out.Add('            last_skin_id: "default",')
$out.Add('        };')
for ($i = $li + 2; $i -lt $lines.Count; $i++) { $out.Add($lines[$i]) }
$lines = $out.ToArray()

# --- 5. self.FIELD -> self.state.FIELD -------------------------------------
$count = 0
for ($i = 0; $i -lt $lines.Count; $i++) {
    $l = $lines[$i]
    if ($l -notmatch 'self\.') { continue }
    foreach ($f in $fields) {
        $pat = 'self\.' + $f + '\b'
        $n = ([regex]::Matches($l, $pat)).Count
        if ($n -gt 0) {
            $l = [regex]::Replace($l, $pat, 'self.state.' + $f)
            $count += $n
        }
    }
    $lines[$i] = $l
}
Write-Host "Замен: $count"

[System.IO.File]::WriteAllLines($path, $lines, (New-Object System.Text.UTF8Encoding($false)))
Write-Host 'Готово. Дальше: cargo build, разбор ошибок, cargo test, cargo clippy.'

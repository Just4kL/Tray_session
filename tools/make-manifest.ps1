<#
  Генератор манифеста обновления update_manifest.json.

  Манифест перечисляет ТОЛЬКО те файлы, которые изменились, вместе с их
  SHA-256. По нему программа решает, что качать, а что уже совпадает, и
  поэтому не трогает остальное — в том числе пользовательские данные
  (sessions.db, config.json, known_games.json, shortcuts.json).

  Запуск из корня проекта, после cargo build --release:
      powershell -ExecutionPolicy Bypass -File tools\make-manifest.ps1
#>

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

# Куда складывать прошлую сборку. Без неё нельзя проверить обновление:
# для этого нужна программа, которую обновление ещё не заменило.
$PrevDir = Join-Path (Get-Location) 'dist\old'

# Сохраняем текущую сборку ДО подмены на новую. Вызывают так:
#     copy target\release\game-session-tracker.exe TraySession.exe
#     tools\make-manifest.ps1 -Archive
# Без -Archive скрипт ничего не архивирует (обычный пересбор манифеста).
if ($args -contains '-Archive') {
    if (Test-Path 'TraySession.exe') {
        $oldVer = 'без-версии'
        if (Test-Path 'update_manifest.json') {
            try {
                $oldVer = (Get-Content 'update_manifest.json' -Raw | ConvertFrom-Json).version
            } catch { $oldVer = 'нечитаемый' }
        }
        New-Item -ItemType Directory -Force -Path $PrevDir | Out-Null
        $dest = Join-Path $PrevDir ("TraySession_$oldVer.exe")
        Copy-Item 'TraySession.exe' $dest -Force
        Write-Host "Прошлая сборка сохранена: $dest"
    }
}

# Файлы сборки, которые имеет смысл обновлять. Порядок не важен.
$Files = @('TraySession.exe', 'Tray_session_setup.exe')

# Достаём версию из Cargo.toml — чтобы манифест и программа не разошлись.
$Cargo = Get-Content Cargo.toml -Raw
if ($Cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    throw 'Не нашёл version в Cargo.toml'
}
$Version = $Matches[1]
$Build = (Get-Date -Format 'yyyyMMdd')

$entries = @()
foreach ($f in $Files) {
    if (-not (Test-Path $f)) {
        Write-Warning "Пропущен $f — файла нет (сначала соберите релиз)"
        continue
    }
    $hash = (Get-FileHash $f -Algorithm SHA256).Hash.ToLower()
    $entries += [ordered]@{
        name   = $f
        sha256 = $hash
    }
    Write-Host ("  {0,-26} {1} байт  {2}" -f $f, (Get-Item $f).Length, $hash.Substring(0, 12))
}

if ($entries.Count -eq 0) {
    throw 'Ни одного файла сборки не найдено — манифест пустой, обновление работать не будет'
}

$manifest = [ordered]@{
    version = $Version
    build   = $Build
    files   = $entries
}

$json = $manifest | ConvertTo-Json -Depth 5
# Windows PowerShell 5.1 пишет BOM, а он ломает разбор JSON — убираем.
[System.IO.File]::WriteAllText(
    (Join-Path (Get-Location) 'update_manifest.json'),
    $json,
    (New-Object System.Text.UTF8Encoding($false))
)

Write-Host ''
Write-Host "Манифест записан: update_manifest.json (версия $Version, сборка $Build)"
Write-Host "Программа читает его по адресу:"
Write-Host "  https://raw.githubusercontent.com/Just4kL/Tray_session/Tray-session/update_manifest.json"

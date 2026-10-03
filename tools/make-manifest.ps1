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

# Сохраняем прошлую сборку и подменяем текущую новой.
#
# Ключевой момент: раньше скрипт ТОЛЬКО архивировал, а новую сборку в корень
# кто-то копировал вручную. Из-за этого однажды манифест был сгенерирован по
# старому файлу, и релиз 0.7.30 объявлял версию 0.7.30, а внутри .exe был
# собран 0.7.29. Оба файла — текстовые, поэтому проверки были довольны.
# Теперь порядок один и он не нарушается: сначала архив, потом копирование.
#
# Вызывают так:
#     cargo build --release
#     tools\make-manifest.ps1 -Archive
#     tools\make-setup.ps1
#     tools\make-manifest.ps1
# Без -Archive скрипт только пересобирает манифест (после make-setup).

if ($args -contains '-Archive') {
    $fresh = Join-Path (Get-Location) 'target\release\game-session-tracker.exe'
    if (-not (Test-Path $fresh)) {
        throw "Нет релизной сборки: $fresh (сначала cargo build --release)"
    }
    $freshHash = (Get-FileHash $fresh -Algorithm SHA256).Hash
    if (Test-Path 'TraySession.exe') {
        $oldVer = 'без-версии'
        if (Test-Path 'update_manifest.json') {
            try {
                $oldVer = (Get-Content 'update_manifest.json' -Raw | ConvertFrom-Json).version
            } catch { $oldVer = 'нечитаемый' }
        }
        New-Item -ItemType Directory -Force -Path $PrevDir | Out-Null
        $dest = Join-Path $PrevDir ("TraySession_$oldVer.exe")
        # ARC-1: в корне уже может лежать СВЕЖАЯ сборка (её кладут до
        # вызова скрипта) — тогда копия из корня это не «прошлая сборка»,
        # а новая под старым именем. Надёжный источник прошлой сборки —
        # git (релизные бинарники коммитятся, HEAD ещё указывает на
        # прошлый релиз). Повторный прогон идемпотентен.
        $rootHash = (Get-FileHash 'TraySession.exe' -Algorithm SHA256).Hash
        if ($rootHash -ne $freshHash) {
            Copy-Item 'TraySession.exe' $dest -Force
            Write-Host "Прошлая сборка сохранена из корня: $dest"
        } else {
            cmd /c "git show HEAD:TraySession.exe > ""$dest""" 2>$null
            if ((Test-Path $dest) -and ((Get-Item $dest).Length -gt 0)) {
                Write-Host "Прошлая сборка взята из git (HEAD:TraySession.exe): $dest"
            } else {
                if (Test-Path $dest) { Remove-Item $dest -Force }
                Write-Warning "git недоступен и корень уже перезаписан — архив пропущен: $dest"
            }
        }
    }
    # Копируем свежую сборку сами: иначе манифест соберётся по старому
    # файлу, который остался в корне от прошлого раза.
    Copy-Item $fresh (Join-Path (Get-Location) 'TraySession.exe') -Force
    # Деактиватор — тот же файл, он узнаёт себя по имени.
    Copy-Item $fresh (Join-Path (Get-Location) '_uninstall.exe') -Force
    Write-Host ("Свежая сборка в корне: {0}" -f (Get-FileHash 'TraySession.exe' -Algorithm SHA256).Hash.ToLower().Substring(0, 12))
}

# Файлы сборки, которые имеет смысл обновлять. Порядок не важен.
$Files = @('TraySession.exe', 'Tray_session_setup.exe')

# Достаём версию из Cargo.toml — чтобы манифест и программа не разошлись.
$Cargo = Get-Content Cargo.toml -Raw
if ($Cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    throw 'Не нашёл version в Cargo.toml'
}
$Version = $Matches[1]
$Build = (Get-Date -Format 'dd.MM.yyyy')

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

# Changelog для диалога подтверждения: секция текущей версии из
# CHANGELOG.md (до следующего заголовка версии), не длиннее ~600 символов.
# Нет файла или секции — null, диалог покажет fallback.
$Changelog = $null
if (Test-Path 'CHANGELOG.md') {
    $cl = Get-Content 'CHANGELOG.md' -Raw
    $esc = [regex]::Escape($Version)
    if ($cl -match "(?ms)^## \[$esc\][^\r\n]*\r?\n(.*?)(?=^## \[|\z)") {
        $t = $Matches[1].Trim()
        if ($t.Length -gt 0) {
            if ($t.Length -gt 600) { $t = $t.Substring(0, 600).Trim() + '…' }
            $Changelog = $t
        }
    }
}

$manifest = [ordered]@{
    version = $Version
    build   = $Build
    files   = $entries
}
if ($null -ne $Changelog) { $manifest['changelog'] = $Changelog }

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

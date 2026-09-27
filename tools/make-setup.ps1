# Сборка установщика Tray_sesstion_setup.exe (7-Zip SFX, только необходимое).
# Состав пакета: TraySession.exe + README.md (конфиги и БД создаются при первом запуске).
$ErrorActionPreference = 'Stop'
$root  = 'F:\Programms\Tray Session python\game-session-tracker'
$seven = 'C:\Program Files\7-Zip\7z.exe'
$dist  = Join-Path $root 'dist\Tray Session'
$exe   = Join-Path $root 'target\release\game-session-tracker.exe'

if (-not (Test-Path $exe)) { throw "Нет релизного exe: $exe (сначала cargo build --release)" }

Remove-Item $dist -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item $exe (Join-Path $dist 'TraySession.exe')
Copy-Item (Join-Path $root 'README.md') $dist

# Максимальное сжатие 7z (solid)
$payload = Join-Path $root 'dist\payload.7z'
Remove-Item $payload -ErrorAction SilentlyContinue
& $seven a -t7z -mx=9 -ms=on $payload (Join-Path $dist '*') | Out-Null

# Склейка GUI SFX-модуля 7-Zip + архив = самораспаковывающийся установщик
$stub = [IO.File]::ReadAllBytes('C:\Program Files\7-Zip\7z.sfx')
$arc  = [IO.File]::ReadAllBytes($payload)
$out  = New-Object byte[] ($stub.Length + $arc.Length)
[Buffer]::BlockCopy($stub, 0, $out, 0, $stub.Length)
[Buffer]::BlockCopy($arc, 0, $out, $stub.Length, $arc.Length)
$setup = Join-Path $root 'Tray_sesstion_setup.exe'
[IO.File]::WriteAllBytes($setup, $out)

Write-Output '=== состав пакета ==='
& $seven l $setup | Select-Object -Last 8
Write-Output '=== проверка целостности ==='
& $seven t $setup | Select-Object -Last 3
Write-Output '=== размеры ==='
Get-ChildItem $setup, (Join-Path $dist 'TraySession.exe') |
    Select-Object Name, @{n='MB'; e={[math]::Round($_.Length / 1MB, 2)}}

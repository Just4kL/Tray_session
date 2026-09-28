# Сборка установщика Tray_session_setup.exe (7-Zip SFX) и резервной копии.
#
# Установщик показывает окно выбора папки: 7-Zip открывает его, когда в
# архиве есть блок настроек `!@Install@!UTF-8!` и в нём НЕТ директивы
# AutoInstall. Раньше блока не было вовсе, поэтому установщик молча
# распаковывался во временный каталог и установки как таковой не было.
#
# Запуск из корня проекта, после cargo build --release:
#     powershell -ExecutionPolicy Bypass -File tools\make-setup.ps1
#
# Файл сохраняется с BOM: Windows PowerShell 5.1 читает .ps1 без BOM как
# ANSI, и кириллица в комментариях и строках превращается в "кавычки",
# которые рвут строки.

$ErrorActionPreference = 'Stop'

# Корень проекта вычисляется от расположения скрипта, а не зашит: иначе
# проект нельзя было бы собрать, перенеся папку или запустив с другого диска.
$root  = Split-Path -Parent $PSScriptRoot
$seven = 'C:\Program Files\7-Zip\7z.exe'
$dist  = Join-Path $root 'dist\Tray Session'
$exe   = Join-Path $root 'target\release\game-session-tracker.exe'
$setup = Join-Path $root 'Tray_session_setup.exe'

if (-not (Test-Path $exe)) { throw "Нет релизного exe: $exe (сначала cargo build --release)" }
if (-not (Test-Path $seven)) { throw "Не найден 7z.exe: $seven" }

# Версия берётся из Cargo.toml, чтобы заголовок установщика не расходился
# с манифестом обновлений.
$cargo = Get-Content (Join-Path $root 'Cargo.toml') -Raw
if ($cargo -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw 'Не нашёл version в Cargo.toml' }
$Version = $Matches[1]

# --- содержимое пакета -------------------------------------------------
Remove-Item $dist -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $dist | Out-Null
Copy-Item $exe (Join-Path $dist 'TraySession.exe')
Copy-Item (Join-Path $root 'README.md') $dist
# Деактиватор: имя с подчёркиванием, чтобы не спутать с программой. Именно
# под этим именем его удаляет сам себя и ищет пользователь.
#
# Это ТОТ ЖЕ файл, что и программа: он узнаёт себя по имени. Отдельного
# маленького бинарника больше нет — он тянул бы за собой копию логирования,
# работы с базой и выгрузки CSV, а они должны быть ровно одни и те же, что и
# у программы. Иначе через полгода эти две копии разошлись бы, и деактиватор
# удалял бы не всё.
Copy-Item $exe (Join-Path $dist '_uninstall.exe')

# Блок настроек SFX. Кладётся в корень архива, иначе 7-Zip его не найдёт.
# AutoInstall здесь НЕТ намеренно — именно его отсутствие включает окно
# выбора папки установки.
#
# RunProgram обязан ссылаться на %%T — это папка, куда РАСПАКОВАНО. Без
# %%T относительный путь разрешается от текущего каталога, а не от
# выбранного пользователем, и запуск сорвётся, как только папку выберут не
# ту, откуда запустили установщик.
#
# Директива Title модулем 7-Zip игнорируется: проверено, заголовок окна
# остаётся "7" и с ASCII, и с кириллицей. Поэтому она не задаётся — иначе
# в коде осталась бы строчка, ни на что не влияющая.
#
# Here-string БЕЗ интерполяции (@'...'@): в блоке нет переменных, а двойные
# кавычки внутри @"..."@ PowerShell искажает, и директива RunProgram
# получается битой.
$config = @'
;!@Install@!UTF-8!
RunProgram="%%T\TraySession.exe"
'@
# Без BOM: имя блока заканчивается на UTF-8, и BOM сломал бы разбор.
[System.IO.File]::WriteAllText(
    (Join-Path $dist '!@Install@!UTF-8!'),
    ($config -replace "`r`n", "`n"),
    (New-Object System.Text.UTF8Encoding($false))
)

# --- сборка архива и SFX ----------------------------------------------
$payload = Join-Path $root 'dist\payload.7z'
Remove-Item $payload -ErrorAction SilentlyContinue
& $seven a -t7z -mx=9 -ms=on $payload (Join-Path $dist '*') | Out-Null

# Склейка модуля SFX + архив = самораспаковывающийся установщик.
$stub = [IO.File]::ReadAllBytes('C:\Program Files\7-Zip\7z.sfx')
$arc  = [IO.File]::ReadAllBytes($payload)
$out  = New-Object byte[] ($stub.Length + $arc.Length)
[Buffer]::BlockCopy($stub, 0, $out, 0, $stub.Length)
[Buffer]::BlockCopy($arc, 0, $out, $stub.Length, $arc.Length)
[IO.File]::WriteAllBytes($setup, $out)

# --- резервная копия ---------------------------------------------------
# Отдельный .7z текущей сборки: откат, если новая версия окажется хуже.
# В репозиторий не попадает (.gitignore), лежит рядом со сборкой.
$stamp  = (Get-Date).ToString('yyyyMMdd')
$backup = Join-Path $root "TraySession-$Version-$stamp.7z"
Remove-Item $backup -ErrorAction SilentlyContinue
& $seven a -t7z -mx=9 -ms=on $backup (Join-Path $dist '*') | Out-Null

Write-Output '=== состав пакета ==='
& $seven l $setup | Select-Object -Last 10
Write-Output '=== проверка целостности ==='
& $seven t $setup | Select-Object -Last 3
& $seven t $backup | Select-Object -Last 2
Write-Output '=== размеры ==='
Get-ChildItem $setup, $backup, (Join-Path $dist 'TraySession.exe') |
    Select-Object Name, @{n='MB'; e={[math]::Round($_.Length / 1MB, 2)}}
Write-Output "=== блок настроек в архиве (без него выбора папки не будет) ==="
& $seven l $setup '!@Install@!UTF-8!' | Select-Object -Last 4

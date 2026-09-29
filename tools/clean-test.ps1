# Вычистить папку тестовых данных temp_test.
#
# Правило владельца проекта: после тестов содержимое удаляется, чтобы не
# оставалось лишних файлов и задвоений данных. Без этого через несколько
# прогонов в папке лежат копии баз и выгрузок, и невозможно понять, какой
# файл от какого теста — а при разборе чужой мусорной базы легко сделать
# неверный вывод о работе программы.
#
# Запуск:
#     powershell -ExecutionPolicy Bypass -File tools\clean-test.ps1
#
# Мусор, который не в temp_test, скрипт НЕ трогает: `target`, сборки,
# пользовательские данные. Здесь только тестовые данные.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$testRoot = Join-Path $root 'temp_test'

if (-not (Test-Path $testRoot)) {
    Write-Host "Папки temp_test нет — чистить нечего."
    return
}

$before = @(Get-ChildItem $testRoot -Recurse -File -ErrorAction SilentlyContinue)
$sizeMB = [math]::Round(($before | Measure-Object Length -Sum).Sum / 1MB, 1)
Write-Host ("Будет удалено: {0} файлов, {1} МБ" -f $before.Count, $sizeMB)

Remove-Item $testRoot -Recurse -Force -ErrorAction SilentlyContinue

if (Test-Path $testRoot) {
    # Что-то держит файл: обычно запущенная программа из этой папки.
    Write-Warning "Не всё удалилось — возможно, из temp_test запущена программа."
    Get-ChildItem $testRoot -Recurse -Force -ErrorAction SilentlyContinue |
        Select-Object -First 10 FullName | ForEach-Object { Write-Warning "  $($_.FullName)" }
} else {
    Write-Host "temp_test удалена."
}

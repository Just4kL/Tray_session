# ONE-SHOT: применён в коммите 1.2.a.4 (см. git log -- tools/).
# Идемпотентен, повторный прогон не нужен.
# 1.2.a.4: thread &Tokens through card/page_title/manual_section callers.
#
# All callers live in TrackerApp methods (&mut self), so an inline shared
# reborrow (self.theme.current().tokens()) is used. If the borrow checker
# complains inside closures that also use self mutably, fix those sites
# by hand with a local `let skin = self.theme.current().clone();`.
#
# Run from project root:
#     powershell -ExecutionPolicy Bypass -File tools\adapt-calls-1.2a.4.ps1

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$path = 'src\app.rs'
$lines = [System.IO.File]::ReadAllLines($path)
$n = 0
for ($i = 0; $i -lt $lines.Count; $i++) {
    $l = $lines[$i]
    if ($l -match '^\s*(pub\s+)?fn\s') { continue }
    $orig = $l
    $l = $l -replace '\bcard\(ui, ', 'card(ui, self.theme.current().tokens(), '
    $l = $l -replace '\bpage_title\(ui, ', 'page_title(ui, self.theme.current().tokens(), '
    $l = $l -replace '\bmanual_section\(ui, ', 'manual_section(ui, self.theme.current().tokens(), '
    if ($l -ne $orig) { $lines[$i] = $l; $n++ }
}
Write-Host "caller lines rewritten: $n"
[System.IO.File]::WriteAllLines($path, $lines, (New-Object System.Text.UTF8Encoding($false)))
Write-Host 'Done. Next: cargo build, fix borrowck conflicts if any.'

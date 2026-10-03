# ONE-SHOT: применён в коммите 1.2.a.4 (см. git log -- tools/).
# Идемпотентен, повторный прогон не нужен.
# 1.2.a.4 follow-up: fix E0500 in closures capturing &mut self.
#
# `card(ui, self.theme.current().tokens(), ...)` inside a `|ui|` closure
# that also calls &mut self methods is a borrowck conflict (shared reborrow
# of self.theme vs unique capture of *self). Fix per enclosing method:
# hoist `let skin = self.theme.current().clone();` (owned Arc, no borrow
# of self) and use `skin.tokens()` in card/page_title/manual_section args.
#
# Run from project root:
#     powershell -ExecutionPolicy Bypass -File tools\fix-skin-1.2a.4.ps1

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$path = 'src\app.rs'
$lines = [System.IO.File]::ReadAllLines($path)

$targets = @('ui_sessions', 'ui_games', 'ui_alarms', 'ui_timer', 'ui_shortcuts', 'about_program')

# Method boundaries: lines starting with exactly 4 spaces + fn.
$bounds = @()
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^    fn (\w+)') { $bounds += @{ idx = $i; name = $Matches[1] } }
}

foreach ($t in $targets) {
    $b = @($bounds | Where-Object { $_.name -eq $t })
    if ($b.Count -eq 0) { throw "method $t not found" }
    $start = $b[0].idx
    $next = $lines.Count
    foreach ($c in $bounds) { if ($c.idx -gt $start -and $c.idx -lt $next) { $next = $c.idx } }
    # Opening brace of the signature (may span lines).
    $open = -1
    for ($i = $start; $i -lt [Math]::Min($start + 10, $next); $i++) {
        if ($lines[$i].TrimEnd().EndsWith('{')) { $open = $i; break }
    }
    if ($open -lt 0) { throw "opening brace for $t not found" }
    # Rewrite matching call args in range.
    for ($i = $open + 1; $i -lt $next; $i++) {
        if ($lines[$i] -match '\b(card|page_title|manual_section)\(') {
            $lines[$i] = $lines[$i] -replace 'self\.theme\.current\(\)\.tokens\(\)', 'skin.tokens()'
        }
    }
    # Insert the binding (after rewrites, so indices below $open are stable;
    # do it via list rebuild at the end per method — simplest: track offset).
    $marker = "        let skin = self.theme.current().clone();"
    $comment = '        // Локальный Arc: замыкания ниже берут &mut self, напрямую через self.theme было бы пересечение заимствований.'
    $lst = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -le $open; $i++) { $lst.Add($lines[$i]) }
    $lst.Add($comment)
    $lst.Add($marker)
    for ($i = $open + 1; $i -lt $lines.Count; $i++) { $lst.Add($lines[$i]) }
    $lines = $lst.ToArray()
    # Recompute bounds after insertion (only this file, rare — recompute all).
    $bounds = @()
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^    fn (\w+)') { $bounds += @{ idx = $i; name = $Matches[1] } }
    }
}
Write-Host 'skin bindings hoisted in: ' + ($targets -join ', ')
[System.IO.File]::WriteAllLines($path, $lines, (New-Object System.Text.UTF8Encoding($false)))
Write-Host 'Done. Next: cargo build.'

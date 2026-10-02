# 1.2.a.3: adapt test call sites to &Tokens signatures.
#
# Only touches lines inside `mod tests` (line >= TESTS_START):
#   - nav_item_paint( -> nav_item_paint(skin.tokens(),  (skip `fn ` defs)
#   - update_notice_paint( -> update_notice_paint(skin.tokens(),  (skip defs)
#   - strip_row single-line `&mut state);` -> `&mut state, skin.tokens());`
#   - strip_row multi-line `&mut state,` -> append `skin.tokens(),` line
#   - insert `let skin = DefaultSkin::default();` once per affected test fn
#
# Run from project root:
#     powershell -ExecutionPolicy Bypass -File tools\adapt-tests-1.2a.3.ps1

$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')
$path = 'src\app.rs'
$lines = [System.IO.File]::ReadAllLines($path)

# Find `mod tests {` line (1-based for messages, 0-based index here).
$testsStart = -1
for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match '^mod tests \{$') { $testsStart = $i; break }
}
if ($testsStart -lt 0) { throw 'mod tests not found' }
Write-Host "mod tests at line $($testsStart + 1)"

$changedFns = New-Object System.Collections.Generic.HashSet[string]
function EnclosingFn($idx) {
    for ($i = $idx; $i -gt $testsStart; $i--) {
        if ($lines[$i] -match '^\s*fn\s+(\w+)') { return $Matches[1] }
    }
    return ''
}

$nCall = 0
for ($i = $testsStart; $i -lt $lines.Count; $i++) {
    $l = $lines[$i]
    if ($l -match '^\s*(pub\s+)?fn\s') { continue }
    $orig = $l
    $l = $l -replace 'nav_item_paint\(', 'nav_item_paint(skin.tokens(), '
    $l = $l -replace 'update_notice_paint\(', 'update_notice_paint(skin.tokens(), '
    if ($l -match 'strip_row\(' -or ($orig -match '^\s*&mut state,$')) {
        # single-line tail
        $l = $l -replace '&mut state\);', '&mut state, skin.tokens());'
        # multi-line tail: `&mut state,` alone on line -> append arg line
        if ($l -match '^(\s*)&mut state,$') {
            $ind = $Matches[1]
            $l = $l + "`r`n" + $ind + 'skin.tokens(),'
        }
    }
    if ($l -ne $orig) {
        $lines[$i] = $l
        $nCall++
        $fn = EnclosingFn $i
        if ($fn -ne '') { [void]$changedFns.Add($fn) }
    }
}
Write-Host "call sites rewritten: $nCall in $($changedFns.Count) fns"

# Insert `let skin` once per affected fn, after its opening brace.
foreach ($fn in $changedFns) {
    # locate `fn NAME`, then first line ending with `{`
    $fi = -1
    for ($i = $testsStart; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match "^\s*fn\s+$fn\b") { $fi = $i; break }
    }
    if ($fi -lt 0) { throw "fn $fn not found" }
    $open = -1
    for ($i = $fi; $i -lt [Math]::Min($fi + 8, $lines.Count); $i++) {
        if ($lines[$i].TrimEnd().EndsWith('{')) { $open = $i; break }
    }
    if ($open -lt 0) { throw "opening brace for $fn not found" }
    # skip if already present (idempotency)
    $has = $false
    for ($i = $open + 1; $i -lt [Math]::Min($open + 6, $lines.Count); $i++) {
        if ($lines[$i] -match 'let skin = DefaultSkin::default\(\);') { $has = $true; break }
        if ($lines[$i] -match '^\s*\}$') { break }
    }
    if (-not $has) {
        $lst = New-Object System.Collections.Generic.List[string]
        for ($i = 0; $i -le $open; $i++) { $lst.Add($lines[$i]) }
        $lst.Add('        let skin = DefaultSkin::default();')
        for ($i = $open + 1; $i -lt $lines.Count; $i++) { $lst.Add($lines[$i]) }
        $lines = $lst.ToArray()
    }
}
Write-Host 'let skin inserted where missing'

[System.IO.File]::WriteAllLines($path, $lines, (New-Object System.Text.UTF8Encoding($false)))
Write-Host 'Done. Next: cargo build, cargo test.'

# Static regression for diff-settings-fixture: one long unchanged line followed
# by one modified line, default dark theme, split mode, no soft wrap.
# Capture a live test window already displaying that fixture's Diff tab.
param([Parameter(Mandatory=$true)][int]$ProcessId,
      [Parameter(Mandatory=$true)][string]$Screenshot)
$ErrorActionPreference = 'Stop'
$p = Get-Process -Id $ProcessId
if ($p.ProcessName -ne 'gitui-diff-tabs') { throw 'Unexpected test process' }
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output $Screenshot | Out-Null
Add-Type -AssemblyName System.Drawing
$b = [System.Drawing.Bitmap]::FromFile($Screenshot)
try {
    $leftRed = 0; $rightGreen = 0; $rightRed = 0; $row = 0
    $start = [int]($b.Width * 0.05)
    $middle = [int]($b.Width * 0.5)
    $end = [int]($b.Width * 0.95)
    for ($y = [int]($b.Height * 0.12); $y -lt [int]($b.Height * 0.3); $y++) {
        $lr = 0; $rg = 0; $rr = 0
        for ($x = $start; $x -lt $end; $x++) {
            $c = $b.GetPixel($x, $y)
            $red = $c.R -gt $c.G * 1.25 -and $c.R -gt $c.B * 1.1 -and $c.R -lt 100
            $green = $c.G -gt $c.R * 1.25 -and $c.G -gt $c.B * 1.1 -and $c.G -lt 100
            if ($x -lt $middle) { if ($red) { $lr++ } }
            else { if ($green) { $rg++ }; if ($red) { $rr++ } }
        }
        if ($lr + $rg -gt $leftRed + $rightGreen) {
            $leftRed = $lr; $rightGreen = $rg; $rightRed = $rr; $row = $y
        }
    }
    $passed = $leftRed -gt ($middle - $start) * 0.8 -and
              $rightGreen -gt ($end - $middle) * 0.8 -and
              $rightRed -lt ($end - $middle) * 0.05
    [ordered]@{ width=$b.Width; height=$b.Height; row=$row;
        leftRed=$leftRed; rightGreen=$rightGreen; rightRed=$rightRed;
        passed=$passed } | ConvertTo-Json
    if (-not $passed) { exit 1 }
} finally { $b.Dispose() }

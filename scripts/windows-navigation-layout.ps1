param([Parameter(Mandatory=$true)][int]$ProcessId,
      [Parameter(Mandatory=$true)][string]$OutputDirectory,
      [ValidateRange(1,20)][int]$Cycles = 3)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing @'
using System;
using System.Drawing;
using System.Collections.Generic;
public static class NavigationBorders {
    public static int[] Read(string path) {
        using (var bitmap = new Bitmap(path)) {
            var result = new List<int>();
            int top = (int)(bitmap.Height * .12), bottom = (int)(bitmap.Height * .40);
            bool inside = false;
            for (int x = 32; x < bitmap.Width - 32; x++) {
                int matches = 0;
                for (int y = top; y < bottom; y++) {
                    var p = bitmap.GetPixel(x,y);
                    if (p.R == 0x26 && p.G == 0x28 && p.B == 0x2c) matches++;
                }
                bool border = matches > (bottom-top)*.85;
                if (border && !inside) result.Add(x);
                inside = border;
            }
            return result.ToArray();
        }
    }
}
'@
$capture = Join-Path $PSScriptRoot 'capture-window.ps1'
Start-Sleep -Seconds 2
$before = Join-Path $OutputDirectory 'before.png'
& $capture -ProcessId $ProcessId -Output $before | Out-Null
$expected = [NavigationBorders]::Read($before)
if ($expected.Count -ne 2) { throw "Expected two navigation dividers; found $($expected -join ',')" }
$results = @()
for ($i=0; $i -lt $Cycles; $i++) {
    & $capture -ProcessId $ProcessId -ClickX 235 -ClickY 87 -Output (Join-Path $OutputDirectory "local-$i.png") | Out-Null
    & $capture -ProcessId $ProcessId -ClickX 110 -ClickY 87 -Output (Join-Path $OutputDirectory "loading-$i.png") | Out-Null
    Start-Sleep -Seconds 2
    $after = Join-Path $OutputDirectory "log-$i.png"
    & $capture -ProcessId $ProcessId -Output $after | Out-Null
    $actual = [NavigationBorders]::Read($after)
    $passed = ($actual.Count -eq $expected.Count)
    if ($passed) {
        for ($j=0; $j -lt $expected.Count; $j++) {
            if ([math]::Abs($actual[$j]-$expected[$j]) -gt 1) { $passed=$false }
        }
    }
    $results += [ordered]@{cycle=$i;expected=$expected;actual=$actual;passed=$passed}
}
$results | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutputDirectory 'results.json')
$results | ConvertTo-Json -Depth 4
if ($results | Where-Object {-not $_.passed}) { throw 'Navigation divider positions changed across tab round trips' }

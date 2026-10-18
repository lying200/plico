# Diagnostic: 100 file selections and 20 refresh clicks.
# In editor-tab mode selection alone does not open/read a Diff. This measures
# list navigation, not 100 editor loads; verify the active layout in captures.
# Focus the second file for alternation, or the first of 100 files for -Sequential.
param([int]$ProcessId, [string]$OutputDirectory, [int]$RefreshX, [int]$RefreshY, [switch]$Sequential)
$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class StressInput {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
[StressInput]::SetThreadDpiAwarenessContext([IntPtr](-4)) | Out-Null
$process = Get-Process -Id $ProcessId
$handle = $process.MainWindowHandle
if ($handle -eq 0) { throw 'No test window' }
$rect = New-Object StressInput+RECT
[StressInput]::GetWindowRect($handle, [ref]$rect) | Out-Null
if ($RefreshX -le 0 -or $RefreshY -le 0 -or $RefreshX -ge ($rect.Right-$rect.Left) -or $RefreshY -ge ($rect.Bottom-$rect.Top)) { throw 'Explicit refresh coordinates within the test window are required' }
$point = New-Object StressInput+POINT
$point.X = $rect.Left + $RefreshX
$point.Y = $rect.Top + $RefreshY
[StressInput]::ScreenToClient($handle, [ref]$point) | Out-Null
$position = [IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
$samples = [System.Collections.Generic.List[object]]::new()
function Sample([string]$phase, [int]$step) {
    $p=Get-Process -Id $ProcessId
    $samples.Add([ordered]@{timestamp=(Get-Date).ToString('o');step=$step;phase=$phase;privateMiB=[math]::Round($p.PrivateMemorySize64/1MB,2);workingSetMiB=[math]::Round($p.WorkingSet64/1MB,2)})
}
Sample 'baseline' 0
for ($i=0; $i -lt 100; $i++) {
    # Address only the test HWND, never send global keys to another application.
    $key=if ($Sequential) { if ($i -eq 0) {0x26} else {0x28} } else { if (($i % 2) -eq 0) {0x26} else {0x28} }
    [StressInput]::PostMessage($handle, 0x0100, [IntPtr]$key, [IntPtr]1) | Out-Null
    [StressInput]::PostMessage($handle, 0x0101, [IntPtr]$key, [IntPtr]1) | Out-Null
    Start-Sleep -Milliseconds 250
    Sample 'file-switch' $i
    if ($i -in @(0,49,99)) {
        & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory "switch-$i.png") | Out-Null
    }
}
for ($i=0; $i -lt 20; $i++) {
    [StressInput]::PostMessage($handle, 0x0201, [IntPtr]1, $position) | Out-Null
    [StressInput]::PostMessage($handle, 0x0202, [IntPtr]::Zero, $position) | Out-Null
    Start-Sleep -Milliseconds 750
    Sample 'refresh' $i
}
for ($i=0; $i -lt 10; $i++) {
    Start-Sleep -Milliseconds 500
    Sample 'settled' $i
}
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory 'final.png') | Out-Null
[ordered]@{
    pid=$ProcessId
    samples=$samples
    mode=if ($Sequential) {'100 sequential file selections'} else {'Two-file alternation'}
    note='Samples exclude child Git and GPU, and do not establish transient peaks. Check intermediate screenshots for actual input response; sequential mode requires the 100-file benchmark. In default editor-tab mode these are list selections, not Diff opens. Refresh clicks are requests, not proof that every asynchronous refresh completed.'
} | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutputDirectory 'stress.json')

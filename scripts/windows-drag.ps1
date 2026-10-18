param(
    [Parameter(Mandatory=$true)][int]$ProcessId,
    [Parameter(Mandatory=$true)][int]$FromX,
    [Parameter(Mandatory=$true)][int]$FromY,
    [Parameter(Mandatory=$true)][int]$ToX,
    [Parameter(Mandatory=$true)][int]$ToY,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ReviewDrag {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
[ReviewDrag]::SetThreadDpiAwarenessContext([IntPtr](-4)) | Out-Null
$process = Get-Process -Id $ProcessId
$handle = $process.MainWindowHandle
if ($handle -eq 0) { throw 'No application window' }
$rect = New-Object ReviewDrag+RECT
if (-not [ReviewDrag]::GetWindowRect($handle, [ref]$rect)) { throw 'Cannot read target bounds' }
function Send-Point([uint32]$Message, [int]$Buttons, [int]$X, [int]$Y) {
    if ($X -lt 0 -or $Y -lt 0 -or $X -ge ($rect.Right-$rect.Left) -or $Y -ge ($rect.Bottom-$rect.Top)) { throw 'Point outside target window' }
    $point = New-Object ReviewDrag+POINT
    $point.X = $rect.Left + $X
    $point.Y = $rect.Top + $Y
    # Hover hit-testing can consult the real pointer during painting. Move only
    # the pointer globally; button events remain addressed to the target HWND.
    if ($Message -eq 0x0200) { [ReviewDrag]::SetCursorPos($point.X, $point.Y) | Out-Null }
    [ReviewDrag]::ScreenToClient($handle, [ref]$point) | Out-Null
    $packed = [IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    if (-not [ReviewDrag]::PostMessage($handle, $Message, [IntPtr]$Buttons, $packed)) { throw 'Target drag message failed' }
}
# Directed HWND messages avoid moving/clicking another application's window.
# Allow hover-dependent controls (auto-hiding scrollbars) to paint before press.
Send-Point 0x0200 0 $FromX $FromY
Start-Sleep -Milliseconds 350
Send-Point 0x0201 1 $FromX $FromY
try {
    Start-Sleep -Milliseconds 100
    for ($step = 1; $step -le 20; $step++) {
        Send-Point 0x0200 1 ([int]($FromX + ($ToX-$FromX)*$step/20)) ([int]($FromY + ($ToY-$FromY)*$step/20))
        Start-Sleep -Milliseconds 30
    }
} finally {
    Send-Point 0x0202 0 $ToX $ToY
}
Start-Sleep -Milliseconds 700
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output $Output

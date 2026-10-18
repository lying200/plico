param([int]$ProcessId, [string]$Output, [string]$Keys = '', [int]$ClickX = -1, [int]$ClickY = -1, [long]$WindowHandle = 0, [int]$WheelDelta = 0, [switch]$HorizontalWheel)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class CaptureWindow {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr h, ref POINT point);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint message, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int command);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
}
'@
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
. (Join-Path $PSScriptRoot 'windows-capture-validation.ps1')
[CaptureWindow]::SetProcessDPIAware() | Out-Null
[CaptureWindow]::SetThreadDpiAwarenessContext([IntPtr](-4)) | Out-Null
$p = Get-Process -Id $ProcessId
$handle = if ($WindowHandle) { [IntPtr]$WindowHandle } else { $p.MainWindowHandle }
if ($handle -eq 0) { throw 'No visible window' }
[CaptureWindow]::ShowWindow($handle, 9) | Out-Null
[CaptureWindow]::SetWindowPos($handle, [IntPtr](-1), 0, 0, 0, 0, 0x43) | Out-Null
$shell = New-Object -ComObject WScript.Shell
$shell.AppActivate($p.Id) | Out-Null
[CaptureWindow]::SetForegroundWindow($handle) | Out-Null
Start-Sleep -Milliseconds 300
if ($ClickX -ge 0) {
    $clickRect = New-Object CaptureWindow+RECT
    [CaptureWindow]::GetWindowRect($handle, [ref]$clickRect) | Out-Null
    [CaptureWindow]::SetCursorPos($clickRect.Left + $ClickX, $clickRect.Top + $ClickY) | Out-Null
    # Address the test window explicitly: global mouse clicks can hit another
    # application if Windows refuses activation or the desktop changes focus.
    $point = New-Object CaptureWindow+POINT
    $point.X = $clickRect.Left + $ClickX
    $point.Y = $clickRect.Top + $ClickY
    [CaptureWindow]::ScreenToClient($handle, [ref]$point) | Out-Null
    $position = [IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
    [CaptureWindow]::PostMessage($handle, 0x0200, [IntPtr]::Zero, $position) | Out-Null
    [CaptureWindow]::PostMessage($handle, 0x0201, [IntPtr]1, $position) | Out-Null
    [CaptureWindow]::PostMessage($handle, 0x0202, [IntPtr]::Zero, $position) | Out-Null
    Start-Sleep -Seconds 1
}
if ($Keys) {
    if ([CaptureWindow]::GetForegroundWindow() -ne $handle) {
        throw 'Target window is not foreground; refusing to send keys to another application'
    }
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Start-Sleep -Seconds 2
}
if ($WheelDelta) {
    if ($ClickX -lt 0 -or [CaptureWindow]::GetForegroundWindow() -ne $handle) {
        throw 'Wheel input requires an explicit click position in the foreground target'
    }
    $wheelData = [BitConverter]::ToUInt32([BitConverter]::GetBytes($WheelDelta), 0)
    $wheelFlag = if ($HorizontalWheel) { 0x1000 } else { 0x0800 }
    [CaptureWindow]::mouse_event($wheelFlag, 0, 0, $wheelData, [UIntPtr]::Zero)
    Start-Sleep -Seconds 1
}
$r = New-Object CaptureWindow+RECT
if (-not [CaptureWindow]::GetWindowRect($handle, [ref]$r)) { return }
if ($r.Right -le $r.Left -or $r.Bottom -le $r.Top) { return }
$b = New-Object System.Drawing.Bitmap ($r.Right-$r.Left),($r.Bottom-$r.Top)
$g = [System.Drawing.Graphics]::FromImage($b)
try {
    $g.CopyFromScreen($r.Left,$r.Top,0,0,$b.Size)
    try { Assert-CaptureVisible $b }
    catch {
        # Keep the failed image under an explicit diagnostic name, never under
        # the success artifact name consumed by the acceptance scripts.
        $b.Save($Output + '.invalid.png')
        if (Test-Path -LiteralPath $Output) { Remove-Item -LiteralPath $Output }
        throw
    }
    $b.Save($Output)
} finally {
    $g.Dispose(); $b.Dispose()
    [CaptureWindow]::SetWindowPos($handle, [IntPtr](-2), 0, 0, 0, 0, 0x43) | Out-Null
}
$p.Refresh()
[ordered]@{pid=$p.Id;title=$p.MainWindowTitle;privateMiB=[math]::Round($p.PrivateMemorySize64/1MB,2);workingSetMiB=[math]::Round($p.WorkingSet64/1MB,2)} | ConvertTo-Json

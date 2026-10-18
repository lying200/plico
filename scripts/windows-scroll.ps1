# Exercise the target app's WM_MOUSEWHEEL path without global wheel input.
param([Parameter(Mandatory=$true)][int]$ProcessId,
      [Parameter(Mandatory=$true)][int]$X,
      [Parameter(Mandatory=$true)][int]$Y,
      [Parameter(Mandatory=$true)][ValidateRange(-32768,32767)][int]$Delta,
      [Parameter(Mandatory=$true)][string]$Output)
$ErrorActionPreference='Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ReviewWheel {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left,Top,Right,Bottom; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
}
'@
[ReviewWheel]::SetThreadDpiAwarenessContext([IntPtr](-4)) | Out-Null
$p=Get-Process -Id $ProcessId
$handle=$p.MainWindowHandle
if($handle -eq 0){throw 'No application window'}
$r=New-Object ReviewWheel+RECT
if(-not [ReviewWheel]::GetWindowRect($handle,[ref]$r)){throw 'Cannot read target bounds'}
if($X -lt 0 -or $Y -lt 0 -or $X -ge ($r.Right-$r.Left) -or $Y -ge ($r.Bottom-$r.Top)){throw 'Point outside target window'}
# WM_MOUSEWHEEL uses screen coordinates, unlike WM_MOUSEMOVE.
$position=[IntPtr]((($r.Top+$Y) -shl 16) -bor (($r.Left+$X) -band 0xffff))
if(-not [ReviewWheel]::PostMessage($handle,0x020A,[IntPtr]($Delta -shl 16),$position)){throw 'Wheel message failed'}
Start-Sleep -Milliseconds 800
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output $Output

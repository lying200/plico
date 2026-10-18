# Fixture: two root files, a-large.rs and z-final.txt, selected commit already loaded.
# Screenshots require manual content inspection; this is not a race-proof assertion.
param([Parameter(Mandatory=$true)][int]$ProcessId,
      [Parameter(Mandatory=$true)][string]$OutputDirectory,
      [int]$FileX=1850, [int]$FirstY=183, [int]$SecondY=225)
$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class RapidFiles {
 [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left,Top,Right,Bottom; }
 [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X,Y; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h,out RECT r);
 [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr h,ref POINT p);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
}
'@
[RapidFiles]::SetThreadDpiAwarenessContext([IntPtr](-4)) | Out-Null
$process=Get-Process -Id $ProcessId
if ($process.ProcessName -ne 'gitui-rapid') { throw 'Expected dedicated gitui-rapid test process' }
$handle=$process.MainWindowHandle
if ($handle -eq 0) { throw 'No test window' }
$rect=New-Object RapidFiles+RECT
[RapidFiles]::GetWindowRect($handle,[ref]$rect) | Out-Null
function ClickFile([int]$y) {
 if ($FileX -lt 0 -or $FileX -ge ($rect.Right-$rect.Left) -or $y -lt 0 -or $y -ge ($rect.Bottom-$rect.Top)) { throw 'Coordinates outside test window' }
 $point=New-Object RapidFiles+POINT
 $point.X=$rect.Left+$FileX; $point.Y=$rect.Top+$y
 [RapidFiles]::ScreenToClient($handle,[ref]$point) | Out-Null
 $position=[IntPtr](($point.Y -shl 16) -bor ($point.X -band 0xffff))
 foreach ($msg in @(0x0200,0x0201,0x0202)) {
  $button=if ($msg -eq 0x0201) {1} else {0}
  [RapidFiles]::PostMessage($handle,$msg,[IntPtr]$button,$position) | Out-Null
 }
}
ClickFile $FirstY
Start-Sleep -Seconds 2
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory 'large.png') | Out-Null
foreach ($delay in @(15,50,100)) {
 for ($i=0;$i -lt 12;$i++) {
  ClickFile $FirstY
  Start-Sleep -Milliseconds $delay
  ClickFile $SecondY
  Start-Sleep -Milliseconds $delay
 }
 Start-Sleep -Seconds 3
 & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory "final-$delay.png") | Out-Null
}
Start-Sleep -Seconds 5
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory 'settled.png') | Out-Null
[ordered]@{pid=$ProcessId;pairs=36;delaysMs=@(15,50,100);expectedFile='z-final.txt';expectedContent='FINAL_SELECTION_SENTINEL';note='Inspect screenshots; input delivery does not prove every request started or completed.'} | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'run.json')

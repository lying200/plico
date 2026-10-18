param([Parameter(Mandatory=$true)][int]$ProcessId,
      [Parameter(Mandatory=$true)][string]$OutputDirectory,
      [double]$MaxPrivateMiB = 150)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ResizeReview {
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int height, uint flags);
    [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
}
'@
[ResizeReview]::SetThreadDpiAwarenessContext([IntPtr](-4)) | Out-Null
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$process = Get-Process -Id $ProcessId
$handle = $process.MainWindowHandle
if ($handle -eq 0) { throw 'No application window' }
$samples = @()
foreach ($size in @(@(1200,800), @(1800,1100), @(1200,800))) {
    if (-not [ResizeReview]::SetWindowPos($handle, [IntPtr]::Zero, 20, 20, $size[0], $size[1], 0x4)) { throw 'Resize failed' }
    Start-Sleep -Seconds 2
    $name = 'resize-' + $samples.Count + '.png'
    & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory $name) | Out-Null
    $process.Refresh()
    if ($process.HasExited) { throw 'Application exited during resize' }
    $samples += [ordered]@{width=$size[0];height=$size[1];privateMiB=[math]::Round($process.PrivateMemorySize64/1MB,2);screenshot=$name}
}
$samples | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'resize.json')
$samples | ConvertTo-Json
if ($samples[-1].privateMiB -gt $MaxPrivateMiB) {
    throw "Post-resize Private Bytes $($samples[-1].privateMiB) MiB exceeds $MaxPrivateMiB MiB budget"
}

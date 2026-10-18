# Uses an isolated slow git.exe to exercise cancellation of a running child.
param([Parameter(Mandatory=$true)][string]$Executable,
      [Parameter(Mandatory=$true)][string]$OutputDirectory,
      [switch]$SwitchRepository)
$ErrorActionPreference = 'Stop'
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path $OutputDirectory) { throw 'Use a new output directory for this test' }
New-Item -ItemType Directory $OutputDirectory | Out-Null
$fakeDirectory = Join-Path $OutputDirectory 'fake-bin'
New-Item -ItemType Directory -Force $fakeDirectory | Out-Null
$fakeExe = Join-Path $fakeDirectory 'git.exe'
if (-not (Test-Path $fakeExe)) {
Add-Type -OutputAssembly $fakeExe -OutputType ConsoleApplication -TypeDefinition @'
using System;
using System.IO;
using System.Diagnostics;
using System.Threading;
public class SlowGit {
 public static void Main(string[] args) {
  File.WriteAllText(Environment.GetEnvironmentVariable("GITUI_TEST_PID_FILE"), Process.GetCurrentProcess().Id.ToString());
  Thread.Sleep(120000);
 }
}
'@
}
$previousPath = $env:PATH
$previousMarker = $env:GITUI_TEST_PID_FILE
$env:PATH = $fakeDirectory + ';' + $env:PATH
$marker = Join-Path $OutputDirectory 'child.pid'
Remove-Item $marker -ErrorAction SilentlyContinue
$env:GITUI_TEST_PID_FILE = $marker
$childId = 0
$app = Start-Process $Executable -ArgumentList ('"' + $OutputDirectory + '"') -PassThru
try {
 $deadline = (Get-Date).AddSeconds(15)
 while (-not (Test-Path $marker)) {
  if ((Get-Date) -gt $deadline) { throw 'Slow Git did not start' }
  Start-Sleep -Milliseconds 100
 }
 $childId = [int](Get-Content $marker -Raw)
 $child = Get-Process -Id $childId
 $deadline = (Get-Date).AddSeconds(15)
 do { $app.Refresh(); if ($app.MainWindowHandle -ne 0) { break }; Start-Sleep -Milliseconds 100 } while ((Get-Date) -lt $deadline)
 $oldChildExited = $null
 if ($SwitchRepository) {
  $secondFolder = Join-Path $OutputDirectory 'second-repository'
  New-Item -ItemType Directory $secondFolder | Out-Null
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $app.Id -ClickX 95 -ClickY 25 -Output (Join-Path $OutputDirectory 'picker.png') | Out-Null
  & (Join-Path $PSScriptRoot 'windows-pick-folder.ps1') -ProcessId $app.Id -Folder $secondFolder
  $oldChildExited = $child.WaitForExit(3000)
  if (-not $oldChildExited) { throw 'Repository switch did not stop the old child' }
  $newChildId = $childId
  $deadline = (Get-Date).AddSeconds(10)
  while ($newChildId -eq $childId -or $newChildId -eq 0) {
   [int]::TryParse((Get-Content $marker -Raw), [ref]$newChildId) | Out-Null
   if ((Get-Date) -gt $deadline) { throw 'Replacement Git task did not start' }
   Start-Sleep -Milliseconds 100
  }
  $childId = $newChildId
  $child = Get-Process -Id $childId
 }
 if (-not $app.CloseMainWindow()) { throw 'CloseMainWindow failed' }
 $appExited = $app.WaitForExit(5000)
 $childExited = $child.WaitForExit(3000)
 $report = [ordered]@{ appId=$app.Id; childId=$childId; appExited=$appExited; childExited=$childExited; oldChildExitedOnSwitch=$oldChildExited; passed=($appExited -and $childExited) }
 $report | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'result.json')
 $report | ConvertTo-Json
 if (-not $report.passed) { throw 'Closing the app left a running process' }
} finally {
 $env:PATH = $previousPath
 $env:GITUI_TEST_PID_FILE = $previousMarker
 if (Test-Path $marker) {
  $lastChildId = 0
  if ([int]::TryParse((Get-Content $marker -Raw), [ref]$lastChildId)) {
   $lastChild = Get-Process -Id $lastChildId -ErrorAction SilentlyContinue
   if ($lastChild -and $lastChild.Path -eq $fakeExe) { Stop-Process -Id $lastChildId -ErrorAction SilentlyContinue }
  }
 }
 if (-not $app.HasExited) { Stop-Process -Id $app.Id -ErrorAction SilentlyContinue }
 if ($childId) { Stop-Process -Id $childId -ErrorAction SilentlyContinue }
}

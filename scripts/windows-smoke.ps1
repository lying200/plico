param(
    [Parameter(Mandatory=$true)][string]$Executable,
    [string]$Repository = '',
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [int]$HistoryLimit = 1000
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$arguments = '--history-limit ' + $HistoryLimit
if ($Repository) { $arguments = '"' + $Repository + '" ' + $arguments }
$process = Start-Process -FilePath $Executable -ArgumentList $arguments -PassThru -RedirectStandardError (Join-Path $OutputDirectory 'stderr.log') -RedirectStandardOutput (Join-Path $OutputDirectory 'stdout.log')
$deadline = (Get-Date).AddSeconds(30)
do {
    Start-Sleep -Milliseconds 250
    $process.Refresh()
    if ($process.HasExited) { throw "Application exited with $($process.ExitCode). See stderr.log" }
} while ($process.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline)
if ($process.MainWindowHandle -eq 0) { throw 'No application window appeared within 30 seconds' }
Start-Sleep -Seconds 3
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $process.Id -Output (Join-Path $OutputDirectory 'window.png') | Out-Null
$process.Refresh()
$report = [ordered]@{
    timestamp = (Get-Date).ToString('o')
    pid = $process.Id
    title = $process.MainWindowTitle
    privateMiB = [math]::Round($process.PrivateMemorySize64 / 1MB, 2)
    workingSetMiB = [math]::Round($process.WorkingSet64 / 1MB, 2)
    peakWorkingSetMiB = [math]::Round($process.PeakWorkingSet64 / 1MB, 2)
    executable = $Executable
    repository = $Repository
    screenshot = 'window.png'
    note = 'Startup sample only; not the 10,000-commit memory acceptance test. GPU memory not measured.'
}
$report | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'startup.json')
$report | ConvertTo-Json

# Two disposable repositories are opened alternately through the real native picker.
param([int]$ProcessId,[string]$RepositoryA,[string]$RepositoryB,[string]$OutputDirectory)
$ErrorActionPreference='Stop'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$samples=@()
for($i=0;$i -lt 10;$i++){
 $folder=if(($i % 2) -eq 0){$RepositoryA}else{$RepositoryB}
 & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -ClickX 80 -ClickY 23 -Output (Join-Path $OutputDirectory 'picker.png') | Out-Null
 & (Join-Path $PSScriptRoot 'windows-pick-folder.ps1') -ProcessId $ProcessId -Folder $folder
 Start-Sleep -Milliseconds 500
 & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory "opened-$i.png") | Out-Null
 $p=Get-Process -Id $ProcessId
 $samples += [ordered]@{step=$i;folder=$folder;privateMiB=[math]::Round($p.PrivateMemorySize64/1MB,2);workingSetMiB=[math]::Round($p.WorkingSet64/1MB,2)}
}
# Cancellation must leave the last repository intact.
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -ClickX 80 -ClickY 23 -Output (Join-Path $OutputDirectory 'picker.png') | Out-Null
& (Join-Path $PSScriptRoot 'windows-pick-folder.ps1') -ProcessId $ProcessId -Cancel
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory 'cancelled.png') | Out-Null
[ordered]@{pid=$ProcessId;samples=$samples;note='Ten in-process repository replacements through the native folder picker. Samples exclude transient peaks, Git children and GPU. Inspect repository names and content in the captures.'} | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutputDirectory 'reopen.json')

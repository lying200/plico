# Use only a disposable 12,000-commit benchmark fixture. The app must already
# show offset 0 with 10,000 commits loaded, at its default 150% DPI window size.
# Creates and removes one temporary ref to test snapshot isolation.
param([Parameter(Mandatory=$true)][int]$ProcessId,
      [Parameter(Mandatory=$true)][string]$Repository,
      [Parameter(Mandatory=$true)][string]$OutputDirectory,
      [ValidateRange(1,10)][int]$Cycles = 4)
$ErrorActionPreference='Stop'
$git='C:\Program Files\Git\cmd\git.exe'
New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
$head = (& $git -C $Repository rev-parse HEAD).Trim()
$older = (& $git -C $Repository rev-list --topo-order --skip=10000 --max-count=1 $head).Trim()
if ($LASTEXITCODE -ne 0 -or $head.Length -ne 40 -or $older.Length -ne 40) { throw 'Invalid benchmark fixture' }
$ref = "refs/heads/gitui-window-validation-$PID"
$created=$false
$samples=@()
$capture=Join-Path $PSScriptRoot 'capture-window.ps1'
try {
    $new = (& $git -C $Repository -c user.name=WindowTest -c user.email=window@example.invalid commit-tree ($head+'^{tree}') -p $head -m 'Temporary snapshot-isolation check').Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Cannot create fixture commit' }
    & $git -C $Repository update-ref $ref $new ('0'*40)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot create temporary fixture ref' }
    $created=$true
    for ($cycle=0; $cycle -lt $Cycles; $cycle++) {
        foreach($direction in @('older','newer')) {
            $expected=if($direction -eq 'older'){$older}else{$head}
            & $capture -ProcessId $ProcessId -ClickX 675 -ClickY 612 -Output (Join-Path $OutputDirectory 'transition.png') | Out-Null
            Start-Sleep -Seconds 2
            # Copy the selected commit ID through the actual app button.
            $name="$cycle-$direction.png"
            & $capture -ProcessId $ProcessId -ClickX 1810 -ClickY 354 -Output (Join-Path $OutputDirectory $name) | Out-Null
            $matches=((Get-Clipboard -Raw).Trim() -eq $expected)
            $p=Get-Process -Id $ProcessId
            $samples += [ordered]@{cycle=$cycle;direction=$direction;expectedCommit=$expected;selectedCommitMatches=$matches;privateMiB=[math]::Round($p.PrivateMemorySize64/1MB,2);workingSetMiB=[math]::Round($p.WorkingSet64/1MB,2);screenshot=$name}
            if(-not $matches){throw "Selected commit mismatch on $direction window at cycle $cycle"}
        }
    }
} finally {
    if($created){
        & $git -C $Repository update-ref -d $ref $new
        if($LASTEXITCODE -ne 0){Write-Warning "Fixture ref cleanup failed: $ref"}
    }
    [ordered]@{samples=$samples;note='Post-transition samples only: not transient peaks, Git child memory, GPU or a long-session leak test. Selected IDs are checked through the UI clipboard; inspect screenshots for count/offset and graph.'} | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutputDirectory 'roundtrip.json')
}
$samples | ConvertTo-Json -Depth 4

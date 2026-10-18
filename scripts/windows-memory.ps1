param([int]$ProcessId, [string]$OutputFile, [int]$Samples=10)
$ErrorActionPreference='Stop'
$values=@()
for ($i=0; $i -lt $Samples; $i++) {
    $p=Get-Process -Id $ProcessId
    $childPrivate=0
    $children=@(Get-CimInstance Win32_Process -Filter "ParentProcessId = $ProcessId")
    foreach ($child in $children) {
        $cp=Get-Process -Id $child.ProcessId -ErrorAction SilentlyContinue
        if ($cp) { $childPrivate += $cp.PrivateMemorySize64 }
    }
    $values += [ordered]@{
        timestamp=(Get-Date).ToString('o')
        privateMiB=[math]::Round($p.PrivateMemorySize64/1MB,2)
        childPrivateMiB=[math]::Round($childPrivate/1MB,2)
        workingSetMiB=[math]::Round($p.WorkingSet64/1MB,2)
    }
    Start-Sleep -Milliseconds 500
}
$gpu=@(Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUProcessMemory -ErrorAction SilentlyContinue | Where-Object { $_.Name -like "pid_${ProcessId}_*" })
$report=[ordered]@{
    pid=$ProcessId
    samples=$values
    otherReviewProcesses=@(Get-Process -Name plico,gitui-benchmark,gitui-welcome,idea64 -ErrorAction SilentlyContinue | Where-Object {$_.Id -ne $ProcessId} | Select-Object Id,ProcessName)
    gpu=@($gpu | Select-Object Name,DedicatedUsage,SharedUsage,TotalCommitted)
    note='A post-startup time series. Samples do not prove a true transient peak; GPU counters reported separately to avoid double counting.'
}
$report | ConvertTo-Json -Depth 5 | Set-Content $OutputFile
$report | ConvertTo-Json -Depth 5

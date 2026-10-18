# Measure native frame availability; this deliberately does not claim repository readiness.
param(
 [Parameter(Mandatory=$true)][string]$Executable,
 [Parameter(Mandatory=$true)][string]$OutputDirectory,
 [string]$Repository='',
 [ValidateRange(1,10)][int]$Runs=5,
 [ValidateRange(1,10000)][int]$HistoryLimit=10000
)
$ErrorActionPreference='Stop'
if(Test-Path $OutputDirectory){throw 'Output directory already exists; preserve earlier evidence'}
New-Item -ItemType Directory $OutputDirectory|Out-Null
$exe=(Resolve-Path $Executable).Path
$profile=Join-Path $OutputDirectory 'profile'
New-Item -ItemType Directory "$profile\GitReview" -Force|Out-Null
Set-Content -Encoding ascii "$profile\GitReview\language" 'en'
$env:LOCALAPPDATA=$profile
$env:PATH='C:\Program Files\Git\cmd;'+$env:PATH
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class StartupProbe {
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll", SetLastError=true)] public static extern IntPtr SendMessageTimeout(IntPtr h,uint msg,UIntPtr w,IntPtr l,uint flags,uint timeout,out UIntPtr result);
}
'@
$samples=@()
for($i=0;$i -lt $Runs;$i++){
 $p=$null
 try {
  $arguments=@('--history-limit',"$HistoryLimit")
  if($Repository){$arguments+=('"'+$Repository+'"')}
  $watch=[Diagnostics.Stopwatch]::StartNew()
  $p=Start-Process $exe -ArgumentList $arguments -PassThru -RedirectStandardError (Join-Path $OutputDirectory "stderr-$i.log")
  $visible=$null;$responsive=$null
  while($watch.Elapsed.TotalSeconds -lt 30){
   $p.Refresh()
   if($p.HasExited){throw "Application exited during startup: $($p.ExitCode)"}
   $h=$p.MainWindowHandle
   if($h -ne [IntPtr]::Zero -and [StartupProbe]::IsWindowVisible($h)){
    if($null -eq $visible){$visible=$watch.Elapsed.TotalMilliseconds}
    $result=[UIntPtr]::Zero
    if([StartupProbe]::SendMessageTimeout($h,0,[UIntPtr]::Zero,[IntPtr]::Zero,2,50,[ref]$result) -ne [IntPtr]::Zero){$responsive=$watch.Elapsed.TotalMilliseconds;break}
   }
   Start-Sleep -Milliseconds 10
  }
  if($null -eq $responsive){throw 'Native frame did not become responsive within 30 seconds'}
  # Post-measurement observation, excluded from the timing. Inspect captures for actual data.
  Start-Sleep -Seconds 3
  if($i -eq 0 -or $i -eq ($Runs-1)){
   & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory "settled-$i.png")|Out-Null
  }
  $p.Refresh()
  $samples+=[ordered]@{run=$i+1;pid=$p.Id;visibleMs=[math]::Round($visible,2);responsiveMs=[math]::Round($responsive,2);settledPrivateBytes=$p.PrivateMemorySize64}
 } finally {
  if($null -ne $p){
   $p.Refresh()
   if(!$p.HasExited){
    if($p.Path -ne $exe){throw 'Owned process identity changed; refusing cleanup'}
    $p.CloseMainWindow()|Out-Null
    if(!$p.WaitForExit(10000)){throw "Owned process $($p.Id) failed to close; no other process was terminated"}
   }
  }
 }
}
[ordered]@{executable=$exe;sha256=(Get-FileHash $exe -Algorithm SHA256).Hash;repository=$Repository;historyLimit=$HistoryLimit;samples=$samples;ownedProcessesExited=$true;note='Stopwatch starts before Start-Process. Visible means nonzero visible MainWindowHandle; responsive additionally requires WM_NULL reply (50ms timeout). Poll 10ms plus scheduling/launch overhead. First run is not a cold-cache boot. Frame readiness is not first paint or Git data readiness. Settled Private excludes Git children and GPU; not a memory peak.'}|ConvertTo-Json -Depth 5|Set-Content (Join-Path $OutputDirectory 'startup.json')

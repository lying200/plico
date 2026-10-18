# Launch suspended, attach to a Job Object, then resume: children are included from startup.
param([Parameter(Mandatory=$true)][string]$Executable,
      [string]$Repository = '', [switch]$Welcome,
      [Parameter(Mandatory=$true)][string]$OutputDirectory,
      [int]$HistoryLimit=10000, [switch]$LargeDiff, [switch]$CombinedStress,
      [string]$ScenarioScript = '', [string]$StartupReport = '', [switch]$ExpectExit, [string]$ExpectedDiffFile = '',
      [ValidateRange(0,100)][int]$DiffNavigationFiles=0,
      [ValidateRange(0,100)][int]$SettingsCycles=0, [int]$DiffFileY=233, [ValidateRange(0,100)][int]$TabCycles=0, [int]$DiffCloseX=642, [ValidateRange(0,100)][int]$RapidTabCycles=0, [ValidateRange(0,100)][int]$SelectionCycles=0, [ValidateRange(3,100)][int]$MemorySamples=20, [switch]$AssertBudget)
$ErrorActionPreference='Stop'
if($StartupReport -and ($StartupReport.Contains('"') -or $StartupReport.EndsWith('\') -or (Test-Path $StartupReport))){throw 'StartupReport must be a new file path without quotes or a trailing separator'}
if($ExpectExit -and !$ScenarioScript){throw 'ExpectExit requires an explicit scenario'}
if($ExpectedDiffFile -and !$LargeDiff){throw 'ExpectedDiffFile requires LargeDiff'}
if($ExpectedDiffFile){
 $ExpectedDiffFile=(Resolve-Path $ExpectedDiffFile).Path
 if((Get-Item $ExpectedDiffFile).Length -gt 5MB){throw 'ExpectedDiffFile exceeds preview budget'}
}
if ($ScenarioScript -and ($LargeDiff -or $CombinedStress -or $SettingsCycles -gt 0 -or $TabCycles -gt 0 -or $RapidTabCycles -gt 0 -or $SelectionCycles -gt 0 -or $DiffNavigationFiles -gt 0)) {
 throw 'ScenarioScript excludes the built-in interaction modes'
}
if ($ScenarioScript) { $ScenarioScript=(Resolve-Path $ScenarioScript).Path }

if ($SettingsCycles -gt 0 -and ($Welcome -or $CombinedStress -or $TabCycles -gt 0 -or $RapidTabCycles -gt 0 -or $SelectionCycles -gt 0 -or $DiffNavigationFiles -gt 0)) {
 throw 'SettingsCycles requires a repository and excludes other interaction modes'
}
if ($DiffNavigationFiles -gt 0 -and (!$LargeDiff -or $Welcome -or $CombinedStress -or $TabCycles -gt 0 -or $RapidTabCycles -gt 0 -or $SelectionCycles -gt 0)) {
 throw 'DiffNavigationFiles requires LargeDiff and excludes other interaction modes'
}
if ($Welcome -and ($Repository -or $LargeDiff -or $CombinedStress -or $TabCycles -gt 0 -or $RapidTabCycles -gt 0 -or $SelectionCycles -gt 0)) {
 throw 'Welcome excludes Repository and repository interaction modes'
}
if ($AssertBudget -and !$Welcome -and !$LargeDiff) { throw 'Budget assertion requires Welcome or LargeDiff' }
if (!$Welcome -and !$Repository) { throw 'Specify Repository or Welcome' }
if ($SelectionCycles -gt 0 -and ($CombinedStress -or $TabCycles -gt 0 -or $RapidTabCycles -gt 0)) {
 throw 'SelectionCycles excludes other stress modes'
}
if ($RapidTabCycles -gt 0 -and (!$LargeDiff -or $CombinedStress -or $TabCycles -gt 0)) { throw 'RapidTabCycles requires LargeDiff and excludes other stress modes' }
if ($TabCycles -gt 0 -and (!$LargeDiff -or $CombinedStress)) { throw 'TabCycles requires LargeDiff and excludes CombinedStress' }
if ($CombinedStress -and ($HistoryLimit -ne 1000 -or $LargeDiff)) {
 throw 'CombinedStress requires HistoryLimit 1000 and no LargeDiff so reopen returns to the same baseline'
}
if (Test-Path $OutputDirectory) { throw 'Use a new output directory' }
New-Item -ItemType Directory $OutputDirectory | Out-Null
Add-Type @'
using System;
using System.Text;
using System.ComponentModel;
using System.Runtime.InteropServices;
public class JobMemory : IDisposable {
 [StructLayout(LayoutKind.Sequential)] public struct Limits {
  public long ProcessTime,JobTime; public uint Flags; public UIntPtr MinWS,MaxWS;
  public uint ActiveLimit; public UIntPtr Affinity; public uint Priority,Scheduling;
 }
 [StructLayout(LayoutKind.Sequential)] public struct IO { public ulong A,B,C,D,E,F; }
 [StructLayout(LayoutKind.Sequential)] public struct Extended {
  public Limits Basic; public IO Io; public UIntPtr ProcessLimit,JobLimit,PeakProcess,PeakJob;
 }
 [StructLayout(LayoutKind.Sequential)] public struct Accounting {
  public long User,Kernel,PeriodUser,PeriodKernel;
  public uint Faults,TotalProcesses,ActiveProcesses,TerminatedProcesses;
 }
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct Startup {
  public uint Size; public string Reserved,Desktop,Title;
  public uint X,Y,XSize,YSize,XChars,YChars,Fill,Flags;
  public ushort Show,ReservedSize; public IntPtr ReservedData,Input,Output,Error;
 }
 [StructLayout(LayoutKind.Sequential)] struct ProcessInfo { public IntPtr Process,Thread; public uint Pid,Tid; }
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateJobObject(IntPtr a,string n);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool SetInformationJobObject(IntPtr j,int c,ref Extended x,uint s);
 [DllImport("kernel32.dll",SetLastError=true,EntryPoint="QueryInformationJobObject")] static extern bool QueryLimits(IntPtr j,int c,out Extended x,uint s,IntPtr r);
 [DllImport("kernel32.dll",SetLastError=true,EntryPoint="QueryInformationJobObject")] static extern bool QueryAccounting(IntPtr j,int c,out Accounting x,uint s,IntPtr r);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcess(string a,StringBuilder cmd,IntPtr pa,IntPtr ta,bool inherit,uint flags,IntPtr env,string cwd,ref Startup si,out ProcessInfo pi);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr j,IntPtr p);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint ResumeThread(IntPtr t);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
 [DllImport("kernel32.dll")] static extern bool TerminateProcess(IntPtr h,uint code);
 [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left,Top,Right,Bottom; }
 [StructLayout(LayoutKind.Sequential)] struct Point { public int X,Y; }
 [DllImport("user32.dll",SetLastError=true)] static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll",SetLastError=true)] static extern bool ScreenToClient(IntPtr h,ref Point p);
 [DllImport("user32.dll",SetLastError=true)] static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h,uint flags);
 [DllImport("user32.dll")] static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
 public static void FocusOwnedWindow(IntPtr h) {
  if(GetForegroundWindow()==h) return;
  SetThreadDpiAwarenessContext(new IntPtr(-4));
  Rect r; Check(GetWindowRect(h,out r));
  var point=new Point { X=r.Left+(r.Right-r.Left)/2,Y=r.Top+25 };
  if(GetAncestor(WindowFromPoint(point),2)!=h) throw new InvalidOperationException("Owned title is obscured; no input sent");
  Check(SetCursorPos(point.X,point.Y));
  mouse_event(2,0,0,0,UIntPtr.Zero); mouse_event(4,0,0,0,UIntPtr.Zero);
  System.Threading.Thread.Sleep(150);
  if(GetForegroundWindow()!=h) throw new InvalidOperationException("Owned window did not receive focus; no keys sent");
 }
 public static void ClickTab(IntPtr h,int x,int y) {
  SetThreadDpiAwarenessContext(new IntPtr(-4));
  Rect r; Check(GetWindowRect(h,out r));
  if(x<0 || y<0 || x>=r.Right-r.Left || y>=r.Bottom-r.Top) throw new ArgumentOutOfRangeException("Tab coordinates");
  var p=new Point { X=r.Left+x,Y=r.Top+y }; Check(ScreenToClient(h,ref p));
  var pos=new IntPtr((p.Y<<16)|(p.X&0xffff));
  Check(PostMessage(h,0x200,IntPtr.Zero,pos));
  Check(PostMessage(h,0x201,new IntPtr(1),pos));
  Check(PostMessage(h,0x202,IntPtr.Zero,pos));
 }
 IntPtr job; public uint Pid;
 static void Check(bool ok) { if (!ok) throw new Win32Exception(Marshal.GetLastWin32Error()); }
 public JobMemory(string exe,string repo,int limit,string report) {
  job=CreateJobObject(IntPtr.Zero,null); Check(job!=IntPtr.Zero);
  ProcessInfo pi=new ProcessInfo();
  try {
   var ext=new Extended(); ext.Basic.Flags=0x2000; // kill test processes if the harness exits
   Check(SetInformationJobObject(job,9,ref ext,(uint)Marshal.SizeOf(typeof(Extended))));
   var si=new Startup(); si.Size=(uint)Marshal.SizeOf(typeof(Startup));
   var cmd=new StringBuilder("\""+exe+"\""+(String.IsNullOrEmpty(repo)?"":" \""+repo+"\"")+" --history-limit "+limit);
   if(!String.IsNullOrEmpty(report)) cmd.Append(" --startup-report \"").Append(report).Append("\"");
   Check(CreateProcess(exe,cmd,IntPtr.Zero,IntPtr.Zero,false,4,IntPtr.Zero,null,ref si,out pi));
   Check(AssignProcessToJobObject(job,pi.Process));
   Check(ResumeThread(pi.Thread)!=0xffffffff); Pid=pi.Pid;
  } catch { if(pi.Process!=IntPtr.Zero) TerminateProcess(pi.Process,1); Dispose(); throw; }
  finally { if(pi.Thread!=IntPtr.Zero) CloseHandle(pi.Thread); if(pi.Process!=IntPtr.Zero) CloseHandle(pi.Process); }
 }
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr h,out uint code);
 public static uint ReadExitCode(IntPtr h) { uint code; Check(GetExitCodeProcess(h,out code)); return code; }
 public Extended ReadLimits() { Extended x; Check(QueryLimits(job,9,out x,(uint)Marshal.SizeOf(typeof(Extended)),IntPtr.Zero)); return x; }
 public Accounting ReadAccounting() { Accounting x; Check(QueryAccounting(job,1,out x,(uint)Marshal.SizeOf(typeof(Accounting)),IntPtr.Zero)); return x; }
 public void Dispose() { if(job!=IntPtr.Zero) { CloseHandle(job); job=IntPtr.Zero; } }
}
'@
$resolvedRepository = if ($Welcome) { '' } else { (Resolve-Path $Repository).Path }
$job=[JobMemory]::new((Resolve-Path $Executable).Path,$resolvedRepository,$HistoryLimit,$StartupReport)
try {
 $p=Get-Process -Id $job.Pid
 $exitHandle=if($ExpectExit){$p.Handle}else{[IntPtr]::Zero}
 $deadline=(Get-Date).AddSeconds(30)
 do { Start-Sleep -Milliseconds 100; $p.Refresh(); if($p.HasExited){throw 'App exited during startup'} } while($p.MainWindowHandle -eq 0 -and (Get-Date) -lt $deadline)
 if($p.MainWindowHandle -eq 0){throw 'No window'}
 Start-Sleep -Seconds 4
 & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory 'history.png') | Out-Null
 if($LargeDiff){
  # Specific to the benchmark and default 150% DPI window; DiffFileY selects its file row.
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX 1850 -ClickY $DiffFileY -Output (Join-Path $OutputDirectory 'selected.png') | Out-Null
  # Targeted window messages avoid global keys when Windows refuses foreground activation.
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX 1708 -ClickY 143 -Output (Join-Path $OutputDirectory 'opened.png') | Out-Null
 }
 Start-Sleep -Seconds 5
 & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory 'window.png') | Out-Null
 $p.Refresh()
 $baselinePrivate=$p.PrivateMemorySize64
 if($DiffNavigationFiles -gt 0) {
  # Default 150% DPI, Chinese UI: editor toolbar previous/next file buttons.
  # Visit each file, then return to the initial file for comparable memory.
  $navigation=@()
  foreach($direction in @('forward','backward')) {
   $x=if($direction -eq 'forward'){304}else{146}
   for($step=1; $step -lt $DiffNavigationFiles; $step++) {
    $before=$job.ReadAccounting().TotalProcesses
    [JobMemory]::ClickTab($p.MainWindowHandle,$x,143)
    $deadline=(Get-Date).AddSeconds(15)
    do {
     Start-Sleep -Milliseconds 100
     $accounting=$job.ReadAccounting()
    } while(($accounting.TotalProcesses -le $before -or $accounting.ActiveProcesses -ne 1) -and (Get-Date) -lt $deadline)
    if($accounting.TotalProcesses -le $before -or $accounting.ActiveProcesses -ne 1){throw "Diff navigation did not load and quiesce: $direction $step"}
    # Allow the completed background result to reach the viewer before next input.
    Start-Sleep -Milliseconds 150
    $p.Refresh()
    $navigation += [ordered]@{direction=$direction;step=$step;newProcesses=$accounting.TotalProcesses-$before;privateBytes=$p.PrivateMemorySize64;peakJobBytes=$job.ReadLimits().PeakJob.ToUInt64()}
    $navigation | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutputDirectory 'diff-navigation.json')
    if($step -eq 1 -or $step % 25 -eq 0 -or $step -eq $DiffNavigationFiles-1) {
     & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory "diff-$direction-$step.png") | Out-Null
     Write-Output "Diff navigation $direction $step"
    }
   }
  }
 }
 if($CombinedStress){
  # Fixed 100-file fixture, default 150% DPI: focus its first file.
  # Default editor-tab mode only selects files; it does not load their Diffs.
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX 1850 -ClickY 183 -Output (Join-Path $OutputDirectory 'baseline.png') | Out-Null
  & (Join-Path $PSScriptRoot 'windows-stress.ps1') -ProcessId $p.Id -OutputDirectory (Join-Path $OutputDirectory 'stress') -RefreshX 2130 -RefreshY 88 -Sequential
  & (Join-Path $PSScriptRoot 'windows-reopen.ps1') -ProcessId $p.Id -RepositoryA $Repository -RepositoryB $Repository -OutputDirectory (Join-Path $OutputDirectory 'reopen')
  Start-Sleep -Seconds 3
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory 'returned.png') | Out-Null
 }
 if($TabCycles -gt 0) {
  $cycles=@()
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX $DiffCloseX -ClickY 77 -Output (Join-Path $OutputDirectory 'cycle-baseline-closed.png') | Out-Null
  Start-Sleep -Seconds 1
  $p.Refresh(); $closedBaseline=$p.PrivateMemorySize64
  for($i=1; $i -le $TabCycles; $i++) {
   $before=$job.ReadAccounting().TotalProcesses
   & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX 1708 -ClickY 143 -Output (Join-Path $OutputDirectory ("cycle-{0:D2}-open.png" -f $i)) | Out-Null
   Start-Sleep -Seconds 1
   $p.Refresh(); $openedPrivate=$p.PrivateMemorySize64; $openedWorking=$p.WorkingSet64
   $accounting=$job.ReadAccounting()
   if($accounting.TotalProcesses -le $before) { throw "Cycle $i did not start a Git read; inspect the screenshots" }
   & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX $DiffCloseX -ClickY 77 -Output (Join-Path $OutputDirectory ("cycle-{0:D2}-closed.png" -f $i)) | Out-Null
   Start-Sleep -Seconds 1
   $p.Refresh()
   $cycles += [ordered]@{cycle=$i; openedPrivateBytes=$openedPrivate; openedWorkingSetBytes=$openedWorking; closedPrivateBytes=$p.PrivateMemorySize64; closedWorkingSetBytes=$p.WorkingSet64; processesStarted=$accounting.TotalProcesses-$before; activeProcessesAfterOpen=$accounting.ActiveProcesses}
   [ordered]@{closedBaselinePrivateBytes=$closedBaseline;cycles=$cycles;note='Fixed benchmark coordinates, one-second settling after each capture. Inspect screenshots; process count confirms a fresh Git read, not visual completion by itself.'} | ConvertTo-Json -Depth 5 | Set-Content (Join-Path $OutputDirectory 'tab-cycles.json')
   if($i % 5 -eq 0) { Write-Output "Completed $i/$TabCycles tab cycles" }
  }
 }
 if($RapidTabCycles -gt 0) {
  $batches=@()
  foreach($delay in @(15,50,150)) {
   $before=$job.ReadAccounting().TotalProcesses
   [JobMemory]::ClickTab($p.MainWindowHandle,85,77)
   Start-Sleep -Milliseconds 200
   $clock=[Diagnostics.Stopwatch]::StartNew()
   for($i=0; $i -lt $RapidTabCycles; $i++) {
    [JobMemory]::ClickTab($p.MainWindowHandle,450,77)
    Start-Sleep -Milliseconds $delay
    [JobMemory]::ClickTab($p.MainWindowHandle,85,77)
    Start-Sleep -Milliseconds $delay
   }
   $clock.Stop()
   Start-Sleep -Seconds 2
   & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory "rapid-$delay-log.png") | Out-Null
   $p.Refresh(); $logPrivate=$p.PrivateMemorySize64
   $afterRapid=$job.ReadAccounting()
   if($afterRapid.TotalProcesses -le $before) { throw 'Rapid switching did not start any Git reads; check coordinates' }
   [JobMemory]::ClickTab($p.MainWindowHandle,450,77)
   Start-Sleep -Seconds 3
   & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory "rapid-$delay-diff.png") | Out-Null
   $p.Refresh()
   $batches += [ordered]@{delayMs=$delay;requestedCycles=$RapidTabCycles;postingElapsedMs=$clock.ElapsedMilliseconds;newProcessesDuringRapid=$afterRapid.TotalProcesses-$before;activeAfterRapid=$afterRapid.ActiveProcesses;logPrivateBytes=$logPrivate;diffPrivateBytes=$p.PrivateMemorySize64;peakJobBytes=$job.ReadLimits().PeakJob.ToUInt64()}
   $batches | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutputDirectory 'rapid-tabs.json')
   Write-Output "Completed $RapidTabCycles rapid Tab cycles at $delay ms"
  }
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX $DiffCloseX -ClickY 77 -Output (Join-Path $OutputDirectory 'rapid-final-closed.png') | Out-Null
  Start-Sleep -Seconds 3
 }
 if($SelectionCycles -gt 0) {
  $selectionBatches=@()
  [JobMemory]::ClickTab($p.MainWindowHandle,85,77)
  Start-Sleep -Seconds 1
  foreach($delay in @(15,50,150)) {
   $before=$job.ReadAccounting().TotalProcesses
   for($i=0; $i -lt $SelectionCycles; $i++) {
    [JobMemory]::ClickTab($p.MainWindowHandle,300,233)
    Start-Sleep -Milliseconds $delay
    [JobMemory]::ClickTab($p.MainWindowHandle,300,191)
    Start-Sleep -Milliseconds $delay
   }
   # Selection queries and diff reads have independent cancellation lifetimes.
   # Wait for every child in this test's job to exit, not merely direct children.
   $deadline=(Get-Date).AddSeconds(15)
   Start-Sleep -Seconds 2
   while($job.ReadAccounting().ActiveProcesses -gt 1 -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 100 }
   $accounting=$job.ReadAccounting()
   if($accounting.ActiveProcesses -ne 1){throw 'Selection queries did not quiesce'}
   if($accounting.TotalProcesses -le $before){throw 'Selection switching started no Git queries; check coordinates'}
   & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory "selection-$delay.png") | Out-Null
   $p.Refresh()
   $selectionBatches += [ordered]@{delayMs=$delay;requestedCycles=$SelectionCycles;newProcesses=$accounting.TotalProcesses-$before;activeProcesses=$accounting.ActiveProcesses;privateBytes=$p.PrivateMemorySize64;workingSetBytes=$p.WorkingSet64;peakJobBytes=$job.ReadLimits().PeakJob.ToUInt64()}
   $selectionBatches | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutputDirectory 'selection-cycles.json')
   Write-Output "Completed $SelectionCycles selection cycles at $delay ms"
  }
  if($LargeDiff) {
   # Re-open the final selected file through its actual toolbar action.
   [JobMemory]::ClickTab($p.MainWindowHandle,1850,$DiffFileY)
   Start-Sleep -Seconds 1
   [JobMemory]::ClickTab($p.MainWindowHandle,1708,143)
   Start-Sleep -Seconds 4
  }
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory 'selection-final.png') | Out-Null
 }
 if($SettingsCycles -gt 0) {
  # Fixed 2182x1361 window at 150% DPI. Preserve every open/closed frame
  # so a missed click cannot silently stand in for a lifecycle exercise.
  $cycles=@()
  for($i=1; $i -le $SettingsCycles; $i++) {
   & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX 1985 -ClickY 30 -Output (Join-Path $OutputDirectory ("settings-{0:D2}-open.png" -f $i)) | Out-Null
   Start-Sleep -Milliseconds 500
   $p.Refresh(); $openedPrivate=$p.PrivateMemorySize64
   & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX 1739 -ClickY 214 -Output (Join-Path $OutputDirectory ("settings-{0:D2}-closed.png" -f $i)) | Out-Null
   Start-Sleep -Milliseconds 500
   $p.Refresh()
   $cycles += [ordered]@{cycle=$i;openedPrivateBytes=$openedPrivate;closedPrivateBytes=$p.PrivateMemorySize64;activeProcesses=$job.ReadAccounting().ActiveProcesses}
   $cycles | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $OutputDirectory 'settings-cycles.json')
   if($i % 5 -eq 0){Write-Output "Completed $i/$SettingsCycles settings cycles"}
  }
 }
 if($ScenarioScript) {
  & $ScenarioScript -ProcessId $p.Id -Job $job -OutputDirectory $OutputDirectory -Repository $resolvedRepository
  if($ExpectExit) {
   if(!$p.WaitForExit(5000)){throw 'Application did not exit after last window closed'}
   $deadline=(Get-Date).AddSeconds(5)
   while($job.ReadAccounting().ActiveProcesses -ne 0 -and (Get-Date)-lt$deadline){Start-Sleep -Milliseconds 100}
   $account=$job.ReadAccounting()
   if($account.ActiveProcesses -ne 0){throw 'Git children remained after natural exit'}
   $exitCode=[JobMemory]::ReadExitCode($exitHandle)
   if($exitCode -ne 0){throw "Application exit code was $exitCode"}
   @{pid=$p.Id;exitCode=$exitCode;naturalExit=$true;activeProcesses=$account.ActiveProcesses;verifiedBeforeJobDisposed=$true}|ConvertTo-Json|Set-Content (Join-Path $OutputDirectory 'natural-exit.json')
   return
  }
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory 'scenario-final.png') | Out-Null
 }
 # Stable-state samples and GPU counters complement the kernel process-group peak.
 # Lifecycle modes finish with the Diff closed. A LargeDiff budget must be
 # sampled with the editor visible, rather than accepting the cheaper Log state.
 $budgetDiffReopened=$false
 if($AssertBudget -and $LargeDiff -and ($TabCycles -gt 0 -or $RapidTabCycles -gt 0)) {
  $before=$job.ReadAccounting().TotalProcesses
  [JobMemory]::ClickTab($p.MainWindowHandle,1708,143)
  $deadline=(Get-Date).AddSeconds(15)
  do {
   Start-Sleep -Milliseconds 100
   $accounting=$job.ReadAccounting()
  } while(($accounting.TotalProcesses -le $before -or $accounting.ActiveProcesses -ne 1) -and (Get-Date) -lt $deadline)
  if($accounting.TotalProcesses -le $before -or $accounting.ActiveProcesses -ne 1){throw 'Budget Diff did not load and quiesce; inspect coordinates'}
  Start-Sleep -Seconds 2
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory 'budget-active-diff.png') | Out-Null
  $budgetDiffReopened=$true
 }
 # A memory-only budget can falsely pass an empty/error view. Optional source
 # verification proves the intended modified column is actually populated.
 if($ExpectedDiffFile){
  [JobMemory]::FocusOwnedWindow($p.MainWindowHandle)
  Set-Clipboard 'gitreview-diff-not-copied'
  & (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -ClickX 1850 -ClickY 300 -Keys '^a^c' -Output (Join-Path $OutputDirectory 'content-verified.png') | Out-Null
  $actual=[string](Get-Clipboard -Raw)
  $expected=[IO.File]::ReadAllText($ExpectedDiffFile)
  if($actual.Replace("`r`n","`n") -cne $expected.Replace("`r`n","`n")){throw 'Requested Diff content is not visible/copyable; memory result is invalid'}
  @{source=$ExpectedDiffFile;bytes=(Get-Item $ExpectedDiffFile).Length;fullCopyMatches=$true;normalization='CRLF only; no trimming'}|ConvertTo-Json|Set-Content (Join-Path $OutputDirectory 'content-verified.json')
  # Clear selection for the steady rendering/memory state without scrolling.
  [JobMemory]::ClickTab($p.MainWindowHandle,1850,300)
 }
 & (Join-Path $PSScriptRoot 'windows-memory.ps1') -ProcessId $p.Id -Samples $MemorySamples -OutputFile (Join-Path $OutputDirectory 'steady.json') | Out-Null
 $limits=$job.ReadLimits(); $account=$job.ReadAccounting(); $p.Refresh()
 $report=[ordered]@{
  executableSha256=(Get-FileHash $Executable -Algorithm SHA256).Hash
  expectedDiffFile=$ExpectedDiffFile
  memorySamples=$MemorySamples; assertBudget=[bool]$AssertBudget; budgetDiffReopened=$budgetDiffReopened
  scenarioScript=$ScenarioScript
  settingsCycles=$SettingsCycles
  selectionCycles=$SelectionCycles
  selectionNote='Cycles alternate the first two history rows at 150% DPI, finish on row one and wait for job children to exit. If LargeDiff is set the final selected file is reopened before steady sampling. No scrolling. Screenshots verify final selection, not every intermediate request.'
  tabCycles=$TabCycles; rapidTabCycles=$RapidTabCycles
  diffNavigationFiles=$DiffNavigationFiles
  diffNavigationNote='Editor previous/next file clicks, each requires new child processes and waits for job quiescence. Returns to first file. Inspect captured file names/counts/content; process counts alone do not prove correct rendering.'
  rapidTabNote='When enabled, cycles are per requested delay (15/50/150 ms). Posted messages and new Git process counts do not prove every request reached parsing. Without AssertBudget, steady.json is sampled after closing the Diff; with AssertBudget, the Diff is reopened for final sampling. rapid-tabs.json contains settled single samples after each batch.'
  baselinePrivateBytes=$baselinePrivate; combinedStress=[bool]$CombinedStress
  combinedStressNote='List-selection, refresh-request and repository-replacement exercise. In editor-tab mode it does not open 100 Diffs. Inspect captures; do not substitute this run for file-content/cache lifecycle acceptance.'
  pid=$p.Id; peakJobBytes=$limits.PeakJob.ToUInt64(); peakAnyProcessBytes=$limits.PeakProcess.ToUInt64()
  totalProcesses=$account.TotalProcesses; activeProcesses=$account.ActiveProcesses
  finalApplicationPrivateBytes=$p.PrivateMemorySize64; finalWorkingSetBytes=$p.WorkingSet64
  historyLimit=$HistoryLimit; repository=$Repository; welcome=[bool]$Welcome; diffFileY=$DiffFileY; largeDiff=[bool]$LargeDiff
  note='Kernel Job Object peak, includes inherited child processes from suspended startup. GPU excluded; not a cold-cache timing test. LargeDiff explicitly opens an editor tab; verify history.png and window.png show the intended history and file; DiffNavigationFiles captures intermediate files and returns to its first file. Coordinates require the fixed benchmark layout at 150% DPI; DiffFileY selects the file row. steady.json contains sampled Private Bytes and separate GPU counters.'
 }
 if($report.peakJobBytes -eq 0){throw 'Job accounting did not record a memory peak'}
 if(!$Welcome -and $report.totalProcesses -lt 2){throw 'Job accounting did not include Git child processes'}
 if($Welcome -and !$ScenarioScript -and $report.totalProcesses -ne 1){throw 'Welcome unexpectedly launched child processes'}
 $report | ConvertTo-Json | Set-Content (Join-Path $OutputDirectory 'memory.json')
 $report | ConvertTo-Json
 if($AssertBudget) {
  $steady=Get-Content (Join-Path $OutputDirectory 'steady.json') -Raw | ConvertFrom-Json
  $maxPrivate=($steady.samples | ForEach-Object {$_.privateMiB + $_.childPrivateMiB} | Measure-Object -Maximum).Maximum
  $stableBudget=if($Welcome){80}else{150}
  if($maxPrivate -gt $stableBudget -or $report.peakJobBytes -gt 250MB) {
   throw "Memory budget failed: stable maximum $maxPrivate MiB (limit $stableBudget), job peak $($report.peakJobBytes / 1MB) MiB (limit 250)"
  }
 }
 $p.CloseMainWindow() | Out-Null
 $p.WaitForExit(5000) | Out-Null
} finally { $job.Dispose() }

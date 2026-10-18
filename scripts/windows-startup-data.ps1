# First-batch readiness, with an actual native selection and full OID readback.
# Fixed 150% DPI layout; no scrolling. Preflight warms Git/filesystem caches.
param(
 [Parameter(Mandatory=$true)][string]$Executable,
 [Parameter(Mandatory=$true)][string]$Repository,
 [Parameter(Mandatory=$true)][string]$OutputDirectory,
 [ValidateRange(1,10)][int]$Runs=5,
 [int]$NodeX=150,[int]$FirstY=188,[int]$ClickX=400
)
$ErrorActionPreference='Stop'
if(Test-Path $OutputDirectory){throw 'Output already exists; preserve earlier evidence'}
New-Item -ItemType Directory $OutputDirectory|Out-Null
$exe=(Resolve-Path $Executable).Path
$git='C:\Program Files\Git\cmd\git.exe'
$head=(& $git -C $Repository rev-parse HEAD)
if($LASTEXITCODE -ne 0){throw 'Cannot resolve fixture HEAD'}
$count=[int](& $git -C $Repository rev-list --count HEAD)
$status=(& $git -C $Repository status --porcelain | Out-String)
if($LASTEXITCODE -ne 0 -or $status -or $count -ne 10000){throw 'Expected clean 10000-commit fixture'}
$profile=Join-Path $OutputDirectory 'profile'
New-Item -ItemType Directory "$profile\GitReview" -Force|Out-Null
Set-Content -Encoding ascii "$profile\GitReview\language" 'en'
Set-Content -Encoding ascii "$profile\GitReview\diff-location" 'tab'
$env:LOCALAPPDATA=$profile
$env:PATH='C:\Program Files\Git\cmd;'+$env:PATH
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class DataStartupProbe {
 [StructLayout(LayoutKind.Sequential)] struct Point {public int X,Y;}
 [StructLayout(LayoutKind.Sequential)] struct Rect {public int Left,Top,Right,Bottom;}
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] static extern bool ScreenToClient(IntPtr h,ref Point p);
 [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h,uint flags);
 [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] static extern bool SetForegroundWindow(IntPtr h);
 [DllImport("user32.dll")] static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
 [DllImport("user32.dll")] static extern bool SetWindowPos(IntPtr h,IntPtr after,int x,int y,int w,int height,uint flags);
 [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr h);
 [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr h,IntPtr dc);
 [DllImport("gdi32.dll")] static extern uint GetPixel(IntPtr dc,int x,int y);
 [DllImport("user32.dll")] static extern bool PostMessageW(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("kernel32.dll")] static extern bool GetExitCodeProcess(IntPtr h,out uint code);
 public static bool Focus(IntPtr h){
  SetThreadDpiAwarenessContext(new IntPtr(-4));
  if(GetForegroundWindow()==h)return true;
  SetWindowPos(h,new IntPtr(-1),0,0,0,0,0x43);SetForegroundWindow(h);
  if(GetForegroundWindow()==h)return true;
  Rect r;if(!GetWindowRect(h,out r))return false;
  var p=new Point{X=(r.Left+r.Right)/2,Y=r.Top+25};
  if(GetAncestor(WindowFromPoint(p),2)!=h)return false;
  SetCursorPos(p.X,p.Y);mouse_event(2,0,0,0,UIntPtr.Zero);mouse_event(4,0,0,0,UIntPtr.Zero);
  return GetForegroundWindow()==h;
 }
 public static bool ReadyPixel(IntPtr h,int x,int y){
  Rect r;if(!GetWindowRect(h,out r))return false;
  var p=new Point{X=r.Left+x,Y=r.Top+y};
  if(GetAncestor(WindowFromPoint(p),2)!=h)return false;
  var dc=GetDC(IntPtr.Zero);try{return GetPixel(dc,p.X,p.Y)==0x73ab6a;}finally{ReleaseDC(IntPtr.Zero,dc);}
 }
 public static void Click(IntPtr h,int x,int y){
  if(GetForegroundWindow()!=h)throw new InvalidOperationException("Owned window is not foreground");
  Rect r;if(!GetWindowRect(h,out r))throw new InvalidOperationException("Window disappeared");
  var p=new Point{X=r.Left+x,Y=r.Top+y};
  if(GetAncestor(WindowFromPoint(p),2)!=h)throw new InvalidOperationException("Click is obscured");
  if(!ScreenToClient(h,ref p))throw new InvalidOperationException("Client conversion failed");
  var pos=new IntPtr((p.Y<<16)|(p.X&0xffff));
  PostMessageW(h,0x200,IntPtr.Zero,pos);PostMessageW(h,0x201,new IntPtr(1),pos);PostMessageW(h,0x202,IntPtr.Zero,pos);
 }
 public static void CheckFocus(IntPtr h){if(GetForegroundWindow()!=h)throw new InvalidOperationException("Foreground changed before copy");}
 public static uint ExitCode(IntPtr h){uint code;if(!GetExitCodeProcess(h,out code))throw new InvalidOperationException("Exit code unavailable");return code;}
}
'@
$samples=@()
for($i=0;$i -lt $Runs;$i++){
 $p=$null
 try {
  $report=Join-Path $OutputDirectory "prepaint-$i.json"
  [Windows.Forms.Clipboard]::SetText("startup-sentinel-$i")
  $watch=[Diagnostics.Stopwatch]::StartNew()
  $p=Start-Process $exe -ArgumentList @('"'+$Repository+'"','--history-limit','1000','--startup-report','"'+$report+'"') -PassThru -RedirectStandardError (Join-Path $OutputDirectory "stderr-$i.log")
  $heldHandle=$p.Handle;$frame=$null;$shown=$null;$clicked=$false;$copied=$null;$lastCopy=-1000
  while($watch.Elapsed.TotalSeconds -lt 30){
   $p.Refresh();if($p.HasExited){throw 'Application exited before readiness'}
   $h=$p.MainWindowHandle
   if($h -ne [IntPtr]::Zero){
    $focused=[DataStartupProbe]::Focus($h)
    if($null -eq $frame -and (Test-Path $report)){
     try{$frame=Get-Content -Raw $report|ConvertFrom-Json}catch{$frame=$null}
     if($null -ne $frame -and ($frame.stage -ne 'history-prepaint' -or $frame.commits -ne 1000 -or $frame.graph_rows -ne 1000 -or $frame.requested -ne 1000 -or $frame.offset -ne 0 -or !$frame.more -or $frame.filtered -or $frame.max_columns -gt 20)){throw 'First batch does not match the ordinary-repository target'}
    }
    if($focused -and $null -ne $frame -and !$clicked -and [DataStartupProbe]::ReadyPixel($h,$NodeX,$FirstY)){
     $shown=$watch.Elapsed.TotalMilliseconds
     [DataStartupProbe]::Click($h,$ClickX,$FirstY);$clicked=$true
    }
    if($clicked){
     if($watch.Elapsed.TotalMilliseconds-$lastCopy -ge 25){[DataStartupProbe]::CheckFocus($h);[Windows.Forms.SendKeys]::SendWait('^+c');$lastCopy=$watch.Elapsed.TotalMilliseconds}
     try{if([Windows.Forms.Clipboard]::GetText() -eq $head){$copied=$watch.Elapsed.TotalMilliseconds;break}}catch{}
    }
   }
   Start-Sleep -Milliseconds 10
  }
  if($null -eq $copied){throw 'No rendered first row plus matching full OID within 30 seconds; inspect coordinates and evidence'}
  if($i -eq 0 -or $i -eq ($Runs-1)){& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $p.Id -Output (Join-Path $OutputDirectory "ready-$i.png")|Out-Null}
  $p.CloseMainWindow()|Out-Null
  if(!$p.WaitForExit(10000)){throw 'Owned application did not close naturally'}
  $exitCode=[DataStartupProbe]::ExitCode($heldHandle)
  if($exitCode -ne 0){throw "Nonzero native exit code $exitCode"}
  $sample=[pscustomobject][ordered]@{run=$i+1;pid=$p.Id;frame=$frame;visibleNodeMs=$shown;interactiveOidMs=$copied;fullOid=$head;exitCode=$exitCode}
  $sample|ConvertTo-Json -Depth 5|Set-Content (Join-Path $OutputDirectory "sample-$i.json")
  $samples+=$sample
 } finally {
  if($null -ne $p){$p.Refresh();if(!$p.HasExited){if($p.Path -ne $exe){throw 'Refusing cleanup of a different executable'};$p.CloseMainWindow()|Out-Null;if(!$p.WaitForExit(10000)){throw 'Owned application remains alive'}}}
 }
}
if((& $git -C $Repository rev-parse HEAD) -ne $head -or (& $git -C $Repository status --porcelain|Out-String) -ne $status){throw 'Readonly fixture changed'}
$pids=@($samples|ForEach-Object{$_.pid})
$children=@(Get-CimInstance Win32_Process|Where-Object{$_.ParentProcessId -in $pids -and $_.Name -eq 'git.exe'})
if($children.Count){throw 'Owned Git child remains alive'}
$maximum=($samples|Measure-Object interactiveOidMs -Maximum).Maximum
[ordered]@{sha256=(Get-FileHash $exe).Hash;repository=$Repository;repositoryCommits=$count;samples=$samples;maximumMs=$maximum;within2seconds=($maximum -le 2000);headUnchanged=$true;statusUnchanged=$true;ownedProcessesExited=$true;note='Warm/uncontrolled filesystem cache after Git preflight; not cold boot. Starts before Start-Process. Opt-in prepaint report proves actual batch counts, not GPU presentation. Screen pixel verifies visible first graph node at fixed 150% layout; native click plus full OID clipboard readback proves interaction. Polling, focus, diagnostics and copy overhead included in conservative upper bound. No scroll. No memory claim.'}|ConvertTo-Json -Depth 7|Set-Content (Join-Path $OutputDirectory 'startup-data.json')
if($maximum -gt 2000){throw 'Observed readiness exceeded two seconds; evidence retained'}

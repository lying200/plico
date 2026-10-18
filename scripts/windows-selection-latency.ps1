# Native Log selection paint timing. Fixed-layout probe, no wheel input.
param(
 [Parameter(Mandatory=$true)][int]$ProcessId,
 [Parameter(Mandatory=$true)][string]$Executable,
 [Parameter(Mandatory=$true)][string]$OutputDirectory,
 [ValidateRange(4,50)][int]$Samples=20,
 [int]$ClickX=600,[int]$ProbeX=1050,[int]$FirstY=190,[int]$SecondY=232
)
$ErrorActionPreference='Stop'
if(Test-Path $OutputDirectory){throw 'Output directory exists'}
New-Item -ItemType Directory $OutputDirectory|Out-Null
$p=Get-Process -Id $ProcessId
$exe=(Resolve-Path $Executable).Path
if($p.Path -ne $exe){throw 'Wrong process identity'}
Add-Type @'
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;
public class LogSelectionProbe {
 [StructLayout(LayoutKind.Sequential)] struct Point { public int X,Y; }
 [StructLayout(LayoutKind.Sequential)] struct Rect { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h,out Rect r);
 [DllImport("user32.dll")] static extern bool ScreenToClient(IntPtr h,ref Point p);
 [DllImport("user32.dll")] static extern IntPtr WindowFromPoint(Point p);
 [DllImport("user32.dll")] static extern IntPtr GetAncestor(IntPtr h,uint flags);
 [DllImport("user32.dll")] static extern short GetAsyncKeyState(int key);
 [DllImport("user32.dll")] static extern IntPtr GetDC(IntPtr h);
 [DllImport("user32.dll")] static extern int ReleaseDC(IntPtr h,IntPtr dc);
 [DllImport("gdi32.dll")] static extern uint GetPixel(IntPtr dc,int x,int y);
 [DllImport("user32.dll")] static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 [DllImport("user32.dll")] static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] static extern void mouse_event(uint flags,uint x,uint y,uint data,UIntPtr extra);
 [DllImport("user32.dll")] static extern IntPtr SetThreadDpiAwarenessContext(IntPtr c);
 public class Sample { public double Milliseconds; public uint BeforeTarget,BeforeOther,AfterTarget,AfterOther; public int Reads; }
 const uint Selected=0x71432a; // COLORREF of the current focused Log selection #2a4371.
 static void Check(bool b,string m) { if(!b)throw new InvalidOperationException(m); }
 static Point At(Rect r,int x,int y) { return new Point{X=r.Left+x,Y=r.Top+y}; }
 static void Owns(IntPtr h,Point point) { Check(GetAncestor(WindowFromPoint(point),2)==h,"Test point is obscured; no input sent"); }
 public static void Focus(IntPtr h) {
  SetThreadDpiAwarenessContext(new IntPtr(-4));
  Rect r;Check(GetWindowRect(h,out r),"No window bounds");
  var title=At(r,(r.Right-r.Left)/2,25); Owns(h,title);
  if(GetForegroundWindow()!=h){SetCursorPos(title.X,title.Y);mouse_event(2,0,0,0,UIntPtr.Zero);mouse_event(4,0,0,0,UIntPtr.Zero);Thread.Sleep(150);}
  Check(GetForegroundWindow()==h,"Test window is not foreground");
 }
 public static Sample ClickAndObserve(IntPtr h,int clickX,int probeX,int targetY,int otherY,bool calibrating) {
  SetThreadDpiAwarenessContext(new IntPtr(-4));
  Rect r;Check(GetWindowRect(h,out r),"No window bounds");
  var click=At(r,clickX,targetY);var target=At(r,probeX,targetY);var other=At(r,probeX,otherY);
  Owns(h,click);Owns(h,target);Owns(h,other);
  Check(GetForegroundWindow()==h,"Test window lost foreground");
  Check((GetAsyncKeyState(16)&0x8000)==0&&(GetAsyncKeyState(17)&0x8000)==0,"Modifier held; no input sent");
  var client=click;Check(ScreenToClient(h,ref client),"Client conversion failed");
  IntPtr dc=GetDC(IntPtr.Zero);Check(dc!=IntPtr.Zero,"No screen DC");
  try {
   var sample=new Sample{BeforeTarget=GetPixel(dc,target.X,target.Y),BeforeOther=GetPixel(dc,other.X,other.Y)};
   if(!calibrating)Check(sample.BeforeTarget!=Selected&&sample.BeforeOther==Selected,"Baseline does not show exactly the opposite row selected");
   var position=new IntPtr((client.Y<<16)|(client.X&0xffff));
   var watch=Stopwatch.StartNew();
   Check(PostMessage(h,0x200,IntPtr.Zero,position),"Move enqueue failed");
   Check(PostMessage(h,0x201,new IntPtr(1),position),"Down enqueue failed");
   Check(PostMessage(h,0x202,IntPtr.Zero,position),"Up enqueue failed");
   do {
    Check(GetForegroundWindow()==h,"Foreground changed during observation");
    Owns(h,target);Owns(h,other);
    sample.AfterTarget=GetPixel(dc,target.X,target.Y);sample.AfterOther=GetPixel(dc,other.X,other.Y);sample.Reads++;
    if(sample.AfterTarget==Selected&&sample.AfterOther!=Selected){sample.Milliseconds=watch.Elapsed.TotalMilliseconds;return sample;}
    Thread.Sleep(1);
   } while(watch.Elapsed.TotalMilliseconds<2000);
   throw new InvalidOperationException("Selection color did not switch in two seconds; inspect layout/colors");
  } finally {ReleaseDC(IntPtr.Zero,dc);}
 }
}
'@
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory 'before.png')|Out-Null
[LogSelectionProbe]::Focus($p.MainWindowHandle)
$calibration=[LogSelectionProbe]::ClickAndObserve($p.MainWindowHandle,$ClickX,$ProbeX,$FirstY,$SecondY,$true)
Start-Sleep -Milliseconds 500
$records=@()
for($i=0;$i -lt $Samples;$i++){
 $target=if($i%2 -eq 0){$SecondY}else{$FirstY};$other=if($i%2 -eq 0){$FirstY}else{$SecondY}
 $result=[LogSelectionProbe]::ClickAndObserve($p.MainWindowHandle,$ClickX,$ProbeX,$target,$other,$false)
 $records+=[ordered]@{sample=$i+1;row=if($i%2 -eq 0){2}else{1};milliseconds=$result.Milliseconds;pixelReads=$result.Reads;beforeTarget=$result.BeforeTarget;beforeOther=$result.BeforeOther;afterTarget=$result.AfterTarget;afterOther=$result.AfterOther}
 Start-Sleep -Milliseconds 250
}
& (Join-Path $PSScriptRoot 'capture-window.ps1') -ProcessId $ProcessId -Output (Join-Path $OutputDirectory 'after.png')|Out-Null
$sorted=@($records|ForEach-Object {$_.milliseconds}|Sort-Object)
[ordered]@{pid=$ProcessId;sha256=(Get-FileHash $exe).Hash;samples=$records;calibration=$calibration;maximumMs=$sorted[-1];p95Ms=$sorted[[math]::Ceiling(0.95*$Samples)-1];within100ms=($sorted[-1] -le 100);clickX=$ClickX;probeX=$ProbeX;rowYs=@($FirstY,$SecondY);note='Posted native mouse messages through a coupled screen-pixel selection-color switch. Poll sleeps 1ms, GetPixel/ownership checks and OS scheduling overhead included; UI event queue plus visible paint, not physical hardware latency, Git readiness or loading-label latency. Twenty ordinary selections, no scroll. Requires current 150% DPI layout and focused #2a4371 selection; mismatched baselines abort.'}|ConvertTo-Json -Depth 6|Set-Content (Join-Path $OutputDirectory 'latency.json')
if($sorted[-1] -gt 100){throw 'At least one observed selection response exceeded 100ms; inspect recorded samples'}

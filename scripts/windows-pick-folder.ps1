# Operate only a native folder dialog owned by the explicit test process.
param([int]$ProcessId, [string]$Folder, [switch]$Cancel)
$ErrorActionPreference='Stop'
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public class FolderPicker {
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
 [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,EntryPoint="SendMessageW")] static extern IntPtr ReadMessage(IntPtr h,uint m,IntPtr w,StringBuilder b);
 public static string Text(IntPtr h){var b=new StringBuilder(32768);ReadMessage(h,0x000D,new IntPtr(b.Capacity),b);return b.ToString();}
 public delegate bool Callback(IntPtr h, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumWindows(Callback cb, IntPtr p);
 [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr h, Callback cb, IntPtr p);
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,StringBuilder s,int n);
 [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,string l);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
}
'@
$dialogs=[System.Collections.Generic.List[long]]::new()
# The shell creates its HWND before showing the initialized picker.
$deadline=(Get-Date).AddSeconds(10)
do {
$dialogs.Clear()
[FolderPicker]::EnumWindows({param($h,$p)
 $owner=0;[FolderPicker]::GetWindowThreadProcessId($h,[ref]$owner)|Out-Null
 if($owner -eq $ProcessId){
  $s=[System.Text.StringBuilder]::new(256);[FolderPicker]::GetClassName($h,$s,256)|Out-Null
  if($s.ToString() -eq '#32770' -and [FolderPicker]::IsWindowVisible($h)){$dialogs.Add($h.ToInt64())}
 };return $true
},[IntPtr]::Zero)|Out-Null
if($dialogs.Count -eq 0){Start-Sleep -Milliseconds 50}
} while($dialogs.Count -eq 0 -and (Get-Date) -lt $deadline)
if($dialogs.Count -ne 1){throw "Expected one native dialog for process $ProcessId, found $($dialogs.Count)"}
$controls=@{}
[FolderPicker]::EnumChildWindows([IntPtr]$dialogs[0],{param($h,$p)
 $id=[FolderPicker]::GetDlgCtrlID($h)
 if($id -in @(1,2,1152)){$controls[$id]=$h};return $true
},[IntPtr]::Zero)|Out-Null
if(!$controls.ContainsKey(1152) -or !$controls.ContainsKey(1) -or !$controls.ContainsKey(2)){throw 'Not the expected folder picker'}
if(!$Cancel){
 if(!(Test-Path -LiteralPath $Folder -PathType Container)){throw 'Fixture folder does not exist'}
 $expected=(Resolve-Path -LiteralPath $Folder).Path
 if(![FolderPicker]::IsWindowEnabled($controls[1152]) -or ![FolderPicker]::IsWindowEnabled($controls[1])){throw 'Native picker controls are not ready'}
 [FolderPicker]::SendMessage($controls[1152],0x000C,[IntPtr]::Zero,$expected)|Out-Null
 # Cross-process edit contents require WM_GETTEXT, not GetWindowText.
 if([FolderPicker]::Text($controls[1152]) -cne $expected){throw 'Native picker did not retain the requested path; no confirmation sent'}
}
$button=if($Cancel){2}else{1}
[FolderPicker]::PostMessage($controls[$button],0x00F5,[IntPtr]::Zero,[IntPtr]::Zero)|Out-Null
Start-Sleep -Milliseconds 1000

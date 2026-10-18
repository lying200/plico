# Literal data entry for the focused GPUI input in an owned foreground window.
# This sends UTF-16 WM_CHAR messages; it is NOT a keyboard or IME test.
# Callers must focus the intended field, wait for delivery, then verify its value.
if (-not ('OwnedLiteralText' -as [type])) {
 Add-Type @'
using System;using System.Runtime.InteropServices;
public class OwnedLiteralText {
 [DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
 [DllImport("user32.dll",EntryPoint="PostMessageW",ExactSpelling=true,SetLastError=true)]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
}
'@
}
function Send-OwnedLiteralText {
 param([Parameter(Mandatory=$true)][IntPtr]$WindowHandle,
       [Parameter(Mandatory=$true)][ValidateRange(1,2147483647)][int]$OwnerProcessId,
       [Parameter(Mandatory=$true)][AllowEmptyString()][string]$Text)
 # Validate before sending anything, so unsupported control keys cannot leave a
 # partially entered value. Use explicit key actions or paste for multiline data.
 foreach($unit in $Text.ToCharArray()) {
  if([char]::IsControl($unit)){throw 'Literal text helper accepts single-line text, not control keys'}
 }
 $nativeOwner=[uint32]0
 [OwnedLiteralText]::GetWindowThreadProcessId($WindowHandle,[ref]$nativeOwner)|Out-Null
 if($nativeOwner-ne$OwnerProcessId){throw 'Literal text target is not the owned process'}
 foreach($unit in $Text.ToCharArray()) {
  if([OwnedLiteralText]::GetForegroundWindow()-ne$WindowHandle){throw 'Literal text target is not foreground'}
  if(![OwnedLiteralText]::PostMessage($WindowHandle,0x102,[IntPtr][int]$unit,[IntPtr]0)){
   throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
  }
 }
}

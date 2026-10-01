param([Parameter(Mandatory)][string]$Script,[string]$OutputDirectory=(Join-Path $env:RUNNER_TEMP 'takedock-unprivileged'))
$ErrorActionPreference='Stop'
# Test-runner process ownership only: this helper is never included in TakeDock.
# WebView2 150+ intentionally rejects environment debugging arguments in elevated hosts.
if(-not $IsWindows){throw 'Windows is required'}
$scriptPath=(Resolve-Path -LiteralPath $Script).Path
$OutputDirectory=[IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $OutputDirectory -Force|Out-Null
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class TakeDockLimitedProcess {
 [StructLayout(LayoutKind.Sequential,CharSet=CharSet.Unicode)] struct STARTUPINFO {
  public int cb; public string reserved, desktop, title;
  public int x,y,xSize,ySize,xChars,yChars,fill,flags; public short show,reserved2;
  public IntPtr bytes,input,output,error;
 }
 [StructLayout(LayoutKind.Sequential)] struct PROCESS_INFORMATION {public IntPtr process,thread;public int pid,tid;}
 [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool CloseHandle(IntPtr handle);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool CreateRestrictedToken(IntPtr token,uint flags,uint disable,IntPtr sids,uint privileges,IntPtr deleted,uint restrict,IntPtr restricted,out IntPtr limited);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessAsUser(IntPtr token,string app,System.Text.StringBuilder command,IntPtr processAttributes,IntPtr threadAttributes,bool inherit,uint flags,IntPtr environment,string directory,ref STARTUPINFO startup,out PROCESS_INFORMATION info);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint WaitForSingleObject(IntPtr handle,uint milliseconds);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint code);
 public static int Run(string application,string command,string directory) {
  IntPtr token=IntPtr.Zero,limited=IntPtr.Zero; PROCESS_INFORMATION info=default;
  try {
   if(!OpenProcessToken(GetCurrentProcess(),0x000F01FF,out token))throw new Win32Exception();
   if(!CreateRestrictedToken(token,4,0,IntPtr.Zero,0,IntPtr.Zero,0,IntPtr.Zero,out limited))throw new Win32Exception();
   var startup=new STARTUPINFO {cb=Marshal.SizeOf<STARTUPINFO>(),flags=1,show=0};
   if(!CreateProcessAsUser(limited,application,new System.Text.StringBuilder(command),IntPtr.Zero,IntPtr.Zero,false,0x08000000,IntPtr.Zero,directory,ref startup,out info))throw new Win32Exception();
   if(WaitForSingleObject(info.process,0xffffffff)!=0)throw new Win32Exception();
   if(!GetExitCodeProcess(info.process,out var code))throw new Win32Exception();
   return unchecked((int)code);
  } finally {foreach(var handle in new[]{info.thread,info.process,limited,token})if(handle!=IntPtr.Zero)CloseHandle(handle);}
 }
}
'@
$child=Join-Path $OutputDirectory 'child.ps1'
$log=Join-Path $OutputDirectory 'acceptance.log'
$escapedScript=$scriptPath.Replace("'","''")
$escapedLog=$log.Replace("'","''")
@"
`$ErrorActionPreference='Stop'
try {
 `$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
 `$principal=[Security.Principal.WindowsPrincipal]::new(`$identity)
 if(`$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Desktop test must run without administrator membership'}
 & '$escapedScript' *> '$escapedLog'
 if(`$LASTEXITCODE){exit `$LASTEXITCODE}
 exit 0
} catch {`$_ | Out-String | Add-Content -LiteralPath '$escapedLog'; exit 1}
"@ | Set-Content -LiteralPath $child
$pwsh=(Get-Command pwsh).Source
$code=[TakeDockLimitedProcess]::Run($pwsh,('"'+$pwsh+'" -NoProfile -File "'+$child+'"'),(Get-Location).Path)
if(Test-Path -LiteralPath $log){Get-Content -LiteralPath $log}
if($code -ne 0){throw "Unprivileged desktop acceptance failed with exit code $code"}

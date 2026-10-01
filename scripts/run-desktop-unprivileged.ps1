param([Parameter(Mandatory)][string]$Script,[string]$OutputDirectory=(Join-Path $env:RUNNER_TEMP 'takedock-unprivileged'))
$ErrorActionPreference='Stop'
# Test-runner process ownership only: this helper is never included in TakeDock.
# WebView2 150+ intentionally rejects environment debugging arguments in elevated hosts.
# Hosted runners use a disposable ordinary account; local runs use the current
# user's filtered token. Never create local accounts on a developer's computer.
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
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr OpenWindowStation(string name,bool inherit,uint access);
 [DllImport("user32.dll",SetLastError=true)] static extern IntPtr GetProcessWindowStation();
 [DllImport("user32.dll",SetLastError=true)] static extern bool SetProcessWindowStation(IntPtr station);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr OpenDesktop(string name,uint flags,bool inherit,uint access);
 [DllImport("user32.dll",SetLastError=true)] static extern bool GetUserObjectSecurity(IntPtr handle,ref uint information,byte[] descriptor,uint length,out uint needed);
 [DllImport("user32.dll",SetLastError=true)] static extern bool SetUserObjectSecurity(IntPtr handle,ref uint information,byte[] descriptor);
 [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr desktop);
 [DllImport("user32.dll")] static extern bool CloseWindowStation(IntPtr station);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessWithLogonW(string user,string domain,string password,uint logon,string app,System.Text.StringBuilder command,uint flags,IntPtr environment,string directory,ref STARTUPINFO startup,out PROCESS_INFORMATION info);
 [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool CloseHandle(IntPtr handle);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateMutex(IntPtr attributes,bool owner,string name);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool GetKernelObjectSecurity(IntPtr handle,uint information,byte[] descriptor,uint length,out uint needed);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool SetKernelObjectSecurity(IntPtr handle,uint information,byte[] descriptor);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool CreateRestrictedToken(IntPtr token,uint flags,uint disable,IntPtr sids,uint privileges,IntPtr deleted,uint restrict,IntPtr restricted,out IntPtr limited);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessAsUser(IntPtr token,string app,System.Text.StringBuilder command,IntPtr processAttributes,IntPtr threadAttributes,bool inherit,uint flags,IntPtr environment,string directory,ref STARTUPINFO startup,out PROCESS_INFORMATION info);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint WaitForSingleObject(IntPtr handle,uint milliseconds);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint code);
 static byte[] GrantDesktopAccess(IntPtr handle,string userSid,int mask) {
  uint information=4,needed;
  GetUserObjectSecurity(handle,ref information,null,0,out needed);
  if(needed==0)throw new Win32Exception();
  var original=new byte[needed];
  if(!GetUserObjectSecurity(handle,ref information,original,needed,out needed))throw new Win32Exception();
  var security=new System.Security.AccessControl.RawSecurityDescriptor(original,0);
  if(security.DiscretionaryAcl==null)return original;
  security.DiscretionaryAcl.InsertAce(security.DiscretionaryAcl.Count,new System.Security.AccessControl.CommonAce(System.Security.AccessControl.AceFlags.None,System.Security.AccessControl.AceQualifier.AccessAllowed,mask,new System.Security.Principal.SecurityIdentifier(userSid),false,null));
  var modified=new byte[security.BinaryLength];security.GetBinaryForm(modified,0);
  if(!SetUserObjectSecurity(handle,ref information,modified))throw new Win32Exception();
  return original;
 }
 static byte[] GrantChromiumStartupAccess(IntPtr handle,string userSid) {
  uint needed;
  GetKernelObjectSecurity(handle,4,null,0,out needed);
  if(needed==0)throw new Win32Exception();
  var original=new byte[needed];
  if(!GetKernelObjectSecurity(handle,4,original,needed,out needed))throw new Win32Exception();
  var security=new System.Security.AccessControl.RawSecurityDescriptor(original,0);
  if(security.DiscretionaryAcl==null)return original;
  security.DiscretionaryAcl.InsertAce(security.DiscretionaryAcl.Count,new System.Security.AccessControl.CommonAce(System.Security.AccessControl.AceFlags.None,System.Security.AccessControl.AceQualifier.AccessAllowed,0x001f0001,new System.Security.Principal.SecurityIdentifier(userSid),false,null));
  var modified=new byte[security.BinaryLength];security.GetBinaryForm(modified,0);
  if(!SetKernelObjectSecurity(handle,4,modified))throw new Win32Exception();
  return original;
 }
 public static int Run(string application,string command,string directory,string user,string password,string userSid,string profile,string temporary) {
  IntPtr token=IntPtr.Zero,limited=IntPtr.Zero,station=IntPtr.Zero,desktop=IntPtr.Zero,startupMutex=IntPtr.Zero; byte[] stationAcl=null,desktopAcl=null,mutexAcl=null; PROCESS_INFORMATION info=default;
  try {
   if(!OpenProcessToken(GetCurrentProcess(),0x000F01FF,out token))throw new Win32Exception();
   if(!CreateRestrictedToken(token,4,0,IntPtr.Zero,0,IntPtr.Zero,0,IntPtr.Zero,out limited))throw new Win32Exception();
   var startup=new STARTUPINFO {cb=Marshal.SizeOf<STARTUPINFO>(),desktop="winsta0\\default",flags=1,show=0};
   if(string.IsNullOrEmpty(user)) {
    if(!CreateProcessAsUser(limited,application,new System.Text.StringBuilder(command),IntPtr.Zero,IntPtr.Zero,false,0x08000000,IntPtr.Zero,directory,ref startup,out info))throw new Win32Exception();
   } else {
   // Hosted disposable VM only: WebView2 needs the interactive station.
   // Preserve its exact DACL, grant only this random test account, restore on exit.
   station=OpenWindowStation("winsta0",false,0x000f037f);
   if(station==IntPtr.Zero)throw new Win32Exception();
   stationAcl=GrantDesktopAccess(station,userSid,0x000f037f);
   var previous=GetProcessWindowStation();
   if(!SetProcessWindowStation(station))throw new Win32Exception();
   try {desktop=OpenDesktop("Default",0,false,0x000f01ff);}
   finally {if(!SetProcessWindowStation(previous))throw new Win32Exception();}
   if(desktop==IntPtr.Zero)throw new Win32Exception();
   desktopAcl=GrantDesktopAccess(desktop,userSid,0x000f01ff);
   // Chromium serializes startup across profiles in the session. A mutex
   // created by runneradmin denies a different ordinary user's CreateMutex.
   // Never acquire it or change ownership; restore its exact DACL on exit.
   startupMutex=CreateMutex(IntPtr.Zero,false,"Local\\ChromeProcessSingletonStartup!");
   if(startupMutex==IntPtr.Zero)throw new Win32Exception();
   mutexAcl=GrantChromiumStartupAccess(startupMutex,userSid);
   var environment=new System.Collections.Generic.SortedDictionary<string,string>(StringComparer.OrdinalIgnoreCase);
   foreach(System.Collections.DictionaryEntry pair in Environment.GetEnvironmentVariables())environment[(string)pair.Key]=(string)pair.Value;
   environment["USERPROFILE"]=profile;environment["APPDATA"]=profile+"\\AppData\\Roaming";environment["LOCALAPPDATA"]=profile+"\\AppData\\Local";environment["USERNAME"]=user;
   environment["TEMP"]=temporary;environment["TMP"]=temporary;
   var block=new System.Text.StringBuilder();
   foreach(var pair in environment)block.Append(pair.Key).Append('=').Append(pair.Value).Append('\0');
   block.Append('\0');
   var env=Marshal.StringToHGlobalUni(block.ToString());
   try {if(!CreateProcessWithLogonW(user,".",password,1,application,new System.Text.StringBuilder(command),0x08000400,env,directory,ref startup,out info))throw new Win32Exception();}
   finally {Marshal.FreeHGlobal(env);}
   }
   if(WaitForSingleObject(info.process,0xffffffff)!=0)throw new Win32Exception();
   if(!GetExitCodeProcess(info.process,out var code))throw new Win32Exception();
   return unchecked((int)code);
  } finally {
   foreach(var handle in new[]{info.thread,info.process,limited,token})if(handle!=IntPtr.Zero)CloseHandle(handle);
   uint information=4;
   bool restored=true;
   if(desktopAcl!=null)restored &= SetUserObjectSecurity(desktop,ref information,desktopAcl);
   if(stationAcl!=null)restored &= SetUserObjectSecurity(station,ref information,stationAcl);
   if(mutexAcl!=null)restored &= SetKernelObjectSecurity(startupMutex,4,mutexAcl);
   if(startupMutex!=IntPtr.Zero)CloseHandle(startupMutex);
   if(desktop!=IntPtr.Zero)CloseDesktop(desktop);
   if(station!=IntPtr.Zero)CloseWindowStation(station);
   if(!restored)throw new Win32Exception("Failed to restore hosted desktop access");
  }
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
if($env:GITHUB_ACTIONS -eq 'true' -and $env:RUNNER_ENVIRONMENT -eq 'github-hosted') {
    $account='tdtest'+([guid]::NewGuid().ToString('N').Substring(0,10))
    $password=[Convert]::ToBase64String([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))+'aA1!'
    $created=$false
    try {
        $user=New-LocalUser -Name $account -Password (ConvertTo-SecureString $password -AsPlainText -Force) -Description 'Disposable TakeDock CI desktop test account'
        $created=$true
        Add-LocalGroupMember -Group (Get-LocalGroup -SID 'S-1-5-32-545') -Member $user
        $acl=Get-Acl -LiteralPath $OutputDirectory
        $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($user.SID,'Modify','ContainerInherit,ObjectInherit','None','Allow'))
        Set-Acl -LiteralPath $OutputDirectory -AclObject $acl
        if($scriptPath -eq (Join-Path $PSScriptRoot 'test-desktop.ps1')) {
            $metadata=& cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
            if($LASTEXITCODE -ne 0){throw 'Cargo metadata failed'}
            & cargo build --locked -p takedock-adb-fixture
            if($LASTEXITCODE -ne 0){throw 'External fixture build failed'}
            $env:TAKEDOCK_PREBUILT_FIXTURE_DIR=$metadata.target_directory
            $env:TAKEDOCK_ACCEPTANCE_COMMIT=(& git rev-parse HEAD)
            if($LASTEXITCODE -ne 0){throw 'Acceptance commit lookup failed'}
            $workspaceAcl=Get-Acl -LiteralPath (Get-Location).Path
            $workspaceAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($user.SID,'Modify','ContainerInherit,ObjectInherit','None','Allow'))
            Set-Acl -LiteralPath (Get-Location).Path -AclObject $workspaceAcl
        }
        $code=[TakeDockLimitedProcess]::Run($pwsh,('"'+$pwsh+'" -NoProfile -File "'+$child+'"'),(Get-Location).Path,$account,$password,$user.SID.Value,"C:\Users\$account",$OutputDirectory)
    } finally {
        if($created){Remove-LocalUser -Name $account}
        $password=$null
    }
} else {
    $code=[TakeDockLimitedProcess]::Run($pwsh,('"'+$pwsh+'" -NoProfile -File "'+$child+'"'),(Get-Location).Path,$null,$null,$null,$null,$null)
}
if(Test-Path -LiteralPath $log){Get-Content -LiteralPath $log}
if($code -ne 0){throw "Unprivileged desktop acceptance failed with exit code $code"}

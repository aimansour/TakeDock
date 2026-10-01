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
 [StructLayout(LayoutKind.Sequential)] struct SECURITY_ATTRIBUTES {public int length;public IntPtr descriptor;public int inherit;}
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool ConvertStringSecurityDescriptorToSecurityDescriptor(string sddl,uint revision,out IntPtr descriptor,out uint size);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateWindowStation(string name,uint flags,uint access,ref SECURITY_ATTRIBUTES attributes);
 [DllImport("user32.dll",SetLastError=true)] static extern IntPtr GetProcessWindowStation();
 [DllImport("user32.dll",SetLastError=true)] static extern bool SetProcessWindowStation(IntPtr station);
 [DllImport("user32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateDesktop(string name,IntPtr device,IntPtr mode,uint flags,uint access,ref SECURITY_ATTRIBUTES attributes);
 [DllImport("user32.dll")] static extern bool CloseDesktop(IntPtr desktop);
 [DllImport("user32.dll")] static extern bool CloseWindowStation(IntPtr station);
 [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr memory);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessWithLogonW(string user,string domain,string password,uint logon,string app,System.Text.StringBuilder command,uint flags,IntPtr environment,string directory,ref STARTUPINFO startup,out PROCESS_INFORMATION info);
 [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool CloseHandle(IntPtr handle);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr process,uint access,out IntPtr token);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool CreateRestrictedToken(IntPtr token,uint flags,uint disable,IntPtr sids,uint privileges,IntPtr deleted,uint restrict,IntPtr restricted,out IntPtr limited);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessAsUser(IntPtr token,string app,System.Text.StringBuilder command,IntPtr processAttributes,IntPtr threadAttributes,bool inherit,uint flags,IntPtr environment,string directory,ref STARTUPINFO startup,out PROCESS_INFORMATION info);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint WaitForSingleObject(IntPtr handle,uint milliseconds);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr process,out uint code);
 public static int Run(string application,string command,string directory,string user,string password,string userSid,string profile,string temporary) {
  IntPtr token=IntPtr.Zero,limited=IntPtr.Zero,descriptor=IntPtr.Zero,station=IntPtr.Zero,desktop=IntPtr.Zero; PROCESS_INFORMATION info=default;
  try {
   if(!OpenProcessToken(GetCurrentProcess(),0x000F01FF,out token))throw new Win32Exception();
   if(!CreateRestrictedToken(token,4,0,IntPtr.Zero,0,IntPtr.Zero,0,IntPtr.Zero,out limited))throw new Win32Exception();
   var startup=new STARTUPINFO {cb=Marshal.SizeOf<STARTUPINFO>(),desktop="winsta0\\default",flags=1,show=0};
   if(user==null) {
    if(!CreateProcessAsUser(limited,application,new System.Text.StringBuilder(command),IntPtr.Zero,IntPtr.Zero,false,0x08000000,IntPtr.Zero,directory,ref startup,out info))throw new Win32Exception();
   } else {
   // A test-owned noninteractive station avoids changing the host desktop ACL.
   string sid=System.Security.Principal.WindowsIdentity.GetCurrent().User.Value;
   if(!ConvertStringSecurityDescriptorToSecurityDescriptor("D:(A;;GA;;;"+sid+")(A;;GA;;;"+userSid+")(A;;GA;;;SY)",1,out descriptor,out var size))throw new Win32Exception();
   var attributes=new SECURITY_ATTRIBUTES {length=Marshal.SizeOf<SECURITY_ATTRIBUTES>(),descriptor=descriptor};
   string name="TakeDockTest-"+Guid.NewGuid().ToString("N");
   station=CreateWindowStation(name,0,0x000f037f,ref attributes);
   if(station==IntPtr.Zero)throw new Win32Exception();
   var previous=GetProcessWindowStation();
   if(!SetProcessWindowStation(station))throw new Win32Exception();
   try {desktop=CreateDesktop("Default",IntPtr.Zero,IntPtr.Zero,0,0x000f01ff,ref attributes);}
   finally {if(!SetProcessWindowStation(previous))throw new Win32Exception();}
   if(desktop==IntPtr.Zero)throw new Win32Exception();
   startup.desktop=name+"\\Default";
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
   if(desktop!=IntPtr.Zero)CloseDesktop(desktop);
   if(station!=IntPtr.Zero)CloseWindowStation(station);
   if(descriptor!=IntPtr.Zero)LocalFree(descriptor);
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
if($env:GITHUB_ACTIONS -eq 'true') {
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

# External infrastructure diagnosis only; never bundled in TakeDock.
$ErrorActionPreference='Stop'
if($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted'){throw 'This probe is restricted to disposable GitHub-hosted Windows VMs'}
$probeRoot=Join-Path $env:RUNNER_TEMP 'webview-host-probe'
New-Item -ItemType Directory -Path $probeRoot -Force|Out-Null
@'
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net10.0-windows10.0.17763.0</TargetFramework><UseWindowsForms>true</UseWindowsForms><PlatformTarget>x64</PlatformTarget></PropertyGroup>
  <ItemGroup><PackageReference Include="Microsoft.Web.WebView2" Version="1.0.4258.31" /></ItemGroup>
</Project>
'@|Set-Content (Join-Path $probeRoot 'Probe.csproj')
@'
using System;
using System.IO;
using System.Diagnostics;
using System.Windows.Forms;
using Microsoft.Web.WebView2.Core;
using Microsoft.Web.WebView2.WinForms;
class Probe {
 [System.Runtime.InteropServices.DllImport("kernel32.dll",CharSet=System.Runtime.InteropServices.CharSet.Unicode,SetLastError=true)] static extern IntPtr CreateMutex(IntPtr attributes,bool owner,string name);
 [System.Runtime.InteropServices.DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [STAThread] static int Main() {
  Console.WriteLine("Identity="+System.Security.Principal.WindowsIdentity.GetCurrent().Name);
  Console.WriteLine("Session="+Process.GetCurrentProcess().SessionId);
  Console.WriteLine("LocalAppData="+Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData));
  Console.WriteLine("UserFolderOverride="+Environment.GetEnvironmentVariable("WEBVIEW2_USER_DATA_FOLDER"));
  var mutex=CreateMutex(IntPtr.Zero,false,@"Local\ChromeProcessSingletonStartup!");
  if(mutex==IntPtr.Zero)Console.WriteLine("ChromiumStartupMutexWin32Error="+System.Runtime.InteropServices.Marshal.GetLastWin32Error());
  else {Console.WriteLine("ChromiumStartupMutex=accessible");CloseHandle(mutex);}
  var directory=Path.Combine(Path.GetTempPath(),"webview-profile-"+Guid.NewGuid());
  Directory.CreateDirectory(directory);
  using(var file=new FileStream(Path.Combine(directory,"lockfile"),FileMode.Create,FileAccess.Write,FileShare.Read,4096,FileOptions.DeleteOnClose))Console.WriteLine("ProfileLockFile=accessible");
  int result=1;
  var form=new Form();
  var view=new WebView2 {Dock=DockStyle.Fill};form.Controls.Add(view);
  var timer=new System.Windows.Forms.Timer {Interval=20000};timer.Tick+=(s,e)=>{Console.WriteLine("Timeout");form.Close();};
  form.Shown+=async (s,e)=> {
   timer.Start();
   try {
    var options=new CoreWebView2EnvironmentOptions("--enable-logging --v=1 --log-file="+Path.Combine(directory,"webview.log"));
    var environment=await CoreWebView2Environment.CreateAsync(null,directory,options);
    await view.EnsureCoreWebView2Async(environment);
    Console.WriteLine("WebViewCreated="+environment.BrowserVersionString);result=0;
   } catch(Exception failure) {Console.WriteLine(failure);}
   finally {timer.Stop();form.Close();}
  };
  Application.Run(form);
  var log=Path.Combine(directory,"webview.log");if(File.Exists(log))Console.WriteLine(File.ReadAllText(log));
  return result;
 }
}
'@|Set-Content (Join-Path $probeRoot 'Program.cs')
& dotnet build (Join-Path $probeRoot 'Probe.csproj') --configuration Release --nologo
if($LASTEXITCODE -ne 0){throw 'WebView host diagnostic build failed'}
$script=Join-Path $probeRoot 'run.ps1'
"& whoami /groups /fo csv; & '$($probeRoot.Replace("'","''"))/bin/Release/net10.0-windows10.0.17763.0/Probe.exe'; exit `$LASTEXITCODE"|Set-Content $script
& "$PSScriptRoot/run-desktop-unprivileged.ps1" -Script $script -OutputDirectory $probeRoot

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
 [STAThread] static int Main() {
  Console.WriteLine("Identity="+System.Security.Principal.WindowsIdentity.GetCurrent().Name);
  Console.WriteLine("Session="+Process.GetCurrentProcess().SessionId);
  Console.WriteLine("LocalAppData="+Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData));
  Console.WriteLine("UserFolderOverride="+Environment.GetEnvironmentVariable("WEBVIEW2_USER_DATA_FOLDER"));
  try {using(var mutex=new System.Threading.Mutex(false,@"Local\ChromeProcessSingletonStartup!"))Console.WriteLine("ChromiumStartupMutex=accessible");}catch(Exception failure){Console.WriteLine("ChromiumStartupMutex="+failure);}
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
$archive=Join-Path $probeRoot 'ProcessMonitor.zip'
Invoke-WebRequest 'https://download.sysinternals.com/files/ProcessMonitor.zip' -OutFile $archive
if((Get-FileHash $archive).Hash -ne '80A6442B46AF762ED1432F6FEC3F7E20366BED62A2522B3486503398A40A1128'){throw 'Diagnostic download checksum changed'}
Expand-Archive -LiteralPath $archive -DestinationPath (Join-Path $probeRoot 'tools')
$monitor=Join-Path $probeRoot 'tools/Procmon64.exe'
$signature=Get-AuthenticodeSignature $monitor
if($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch '^CN=Microsoft Corporation,'){throw 'Invalid Microsoft diagnostic signature'}
$trace=Join-Path $probeRoot 'trace.pml'
$csv=Join-Path $probeRoot 'trace.csv'
Start-Process -FilePath $monitor -ArgumentList @('/AcceptEula','/Quiet','/Minimized','/BackingFile',('"'+$trace+'"')) -WindowStyle Hidden|Out-Null
$idle=Start-Process -FilePath $monitor -ArgumentList '/WaitForIdle' -WindowStyle Hidden -PassThru
if(-not $idle.WaitForExit(15000)){Stop-Process -Id $idle.Id; Write-Output 'Diagnostic monitor readiness timed out'}
try {& "$PSScriptRoot/run-desktop-unprivileged.ps1" -Script $script -OutputDirectory $probeRoot}
finally {
 & $monitor /Terminate
 $export=Start-Process -FilePath $monitor -ArgumentList @('/AcceptEula','/Quiet','/OpenLog',('"'+$trace+'"'),'/SaveAs',('"'+$csv+'"')) -WindowStyle Hidden -PassThru
 if(-not $export.WaitForExit(30000)){Stop-Process -Id $export.Id; Write-Output 'Diagnostic monitor export timed out'}
 if(Test-Path $csv) {
  # Process-start records can contain inherited environment secrets. Export only
  # file/registry results and paths, never command lines or environment blocks.
  Import-Csv -LiteralPath $csv | Where-Object {$_.'Process Name' -in @('Probe.exe','msedgewebview2.exe') -and ($_.Result -eq 'ACCESS DENIED' -or $_.Path -match 'lockfile') -and $_.Operation -notin @('Process Start','Process Create')} | Select-Object 'Process Name',PID,Operation,Path,Result | Export-Csv -LiteralPath (Join-Path $probeRoot 'safe-results.csv') -NoTypeInformation
  Get-Content -LiteralPath (Join-Path $probeRoot 'safe-results.csv') -Tail 60
 }
}

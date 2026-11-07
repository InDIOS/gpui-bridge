# RAM soak: runs benchmarks/churn.mjs and prints the host's memory every few seconds. The numbers
# must level off after the first seconds; a steady climb is a leak.
#
#   pwsh -File benchmarks/soak.ps1 -ScriptArgs "300 5" -Every 15
#   pwsh -File benchmarks/soak.ps1 -ScriptArgs "60 20 image" -HostPath target/release/gpui-bridge-host.exe

param([string]$ScriptArgs = "300 5", [int]$Every = 15, [string]$HostPath, [string]$Script = "benchmarks/churn.mjs")

$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)
$known = @(Get-Process gpui-bridge-host -ErrorAction SilentlyContinue | ForEach-Object Id)
$psi = [Diagnostics.ProcessStartInfo]::new()
$psi.FileName = "node"
$psi.Arguments = "`"$Script`" " + $ScriptArgs
$psi.WorkingDirectory = (Get-Location).Path   # Set-Location does not move the .NET current directory
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
if ($HostPath) { $psi.Environment["GPUI_BRIDGE_HOST"] = (Resolve-Path $HostPath).Path }
$p = [Diagnostics.Process]::Start($psi)
$err = $p.StandardError.ReadToEndAsync()
$null = $p.StandardOutput.ReadLine()   # READY
$h = Get-Process gpui-bridge-host | Where-Object { $known -notcontains $_.Id } | Sort-Object StartTime -Descending | Select-Object -First 1
$t0 = Get-Date
$out = $p.StandardOutput.ReadLineAsync()
while (-not $out.IsCompleted) {
  $h.Refresh()
  if ($h.HasExited) { break }
  "{0,5:N0}s ws={1,7:N1} MB private={2,7:N1} MB handles={3}" -f ((Get-Date) - $t0).TotalSeconds, ($h.WorkingSet64/1MB), ($h.PrivateMemorySize64/1MB), $h.HandleCount
  Start-Sleep -Seconds $Every
}
$out.Result
$p.WaitForExit()
($err.Result -split "`r?`n" | Where-Object { $_ -match "ERR|panick" } | Select-Object -First 12) -join "`n"

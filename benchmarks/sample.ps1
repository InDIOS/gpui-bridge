# One render sample by hand: starts benchmarks/render.mjs, waits for READY, then reports the host's CPU
# over N seconds of steady state and, with -Log, the host's stderr (frames/s with RUST_LOG=debug).
#
#   pwsh -File benchmarks/sample.ps1 -Nodes 20000 -Mode spin -Seconds 5 -Log

param(
  [int]$Nodes = 20000,
  [ValidateSet("idle", "spin")][string]$Mode = "spin",
  [int]$Seconds = 5,
  [switch]$Log,
  [string]$HostPath
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$known = @(Get-Process -Name "gpui-bridge-host" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
$psi = [Diagnostics.ProcessStartInfo]::new()
$psi.FileName = "node"
$psi.Arguments = "benchmarks/render.mjs $Nodes $Mode"
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
if ($Log) {
  $psi.RedirectStandardError = $true
  $psi.Environment["RUST_LOG"] = "debug"
}
if ($HostPath) { $psi.Environment["GPUI_BRIDGE_HOST"] = (Resolve-Path $HostPath).Path }
$process = [Diagnostics.Process]::Start($psi)

$null = $process.StandardOutput.ReadLine()   # READY
$hostProcess = Get-Process -Name "gpui-bridge-host" -ErrorAction SilentlyContinue |
  Where-Object { $known -notcontains $_.Id } | Sort-Object StartTime -Descending | Select-Object -First 1
if (-not $hostProcess) { Write-Host "host not found"; $process.Kill(); exit 1 }

$hostProcess.Refresh()
$rss = [math]::Round($hostProcess.WorkingSet64 / 1MB, 1)
$before = $hostProcess.TotalProcessorTime
Start-Sleep -Seconds $Seconds
$hostProcess.Refresh()
$cpu = [math]::Round(($hostProcess.TotalProcessorTime - $before).TotalSeconds / $Seconds * 100, 1)
Write-Host "nodes=$Nodes mode=$Mode rss=$rss MB cpu=$cpu %"
if ($Log) {
  $stderr = $process.StandardError.ReadToEndAsync()
  Start-Sleep -Milliseconds 300
  $process.Kill()
  $process.WaitForExit()
  $lines = $stderr.Result -split "`r?`n" | Where-Object { $_ -match "frames/s" }
  Write-Host "--- host frames/s (last 12 lines) ---"
  $lines | Select-Object -Last 12
} else {
  $process.Kill()
  $process.WaitForExit()
}

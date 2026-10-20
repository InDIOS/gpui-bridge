# Real-window check for the incremental redraw: starts an example, reports the host's CPU and its
# frames/s with nothing touched, then moves the pointer to a point inside the window (its own
# `hover:` transition is then the only thing moving) and reports again. The pointer goes back where
# it was. The window is the one the example opens, centered; `-Width`, `-Height` and the point are
# the example's own.
#
#   pwsh -File benchmarks/hover-check.ps1                                                 # examples/animations.ts, a hover row
#   pwsh -File benchmarks/hover-check.ps1 -Example examples/data.ts -Width 720 -Height 560 -AtX 300 -AtY 150

param(
  [string]$Example = "examples/animations.ts",
  [int]$Width = 560,
  [int]$Height = 460,
  [int]$AtX = 184,
  [int]$AtY = 138,
  [string]$HostPath
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Cursor {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
}
"@

$was = [Cursor+POINT]::new()
$null = [Cursor]::GetCursorPos([ref]$was)

$env:RUST_LOG = "debug"
$psi = [Diagnostics.ProcessStartInfo]::new()
$psi.FileName = "node"
$psi.Arguments = "$Example --debug"
$psi.WorkingDirectory = $root
$psi.UseShellExecute = $false
$psi.RedirectStandardError = $true
if ($HostPath) { $psi.Environment["GPUI_BRIDGE_HOST"] = (Resolve-Path $HostPath).Path }
$process = [Diagnostics.Process]::Start($psi)
$stderr = $process.StandardError.ReadToEndAsync()

Start-Sleep -Seconds 6
$hostProcess = Get-Process -Name "gpui-bridge-host" -ErrorAction SilentlyContinue | Sort-Object StartTime -Descending | Select-Object -First 1
if (-not $hostProcess) { Write-Host "host not found"; $process.Kill(); exit 1 }

function Sample([string]$label) {
  $hostProcess.Refresh()
  $before = $hostProcess.TotalProcessorTime
  Start-Sleep -Seconds 2
  $hostProcess.Refresh()
  Write-Host ("{0}: {1:N1} % of a core" -f $label, (($hostProcess.TotalProcessorTime - $before).TotalSeconds / 2 * 100))
}

Sample "nothing moving but the example's loops"

Add-Type -AssemblyName System.Windows.Forms
$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$x = [int]($screen.Width / 2 - $Width / 2 + $AtX)
$y = [int]($screen.Height / 2 - $Height / 2 + $AtY)
# A small wiggle first: a move to where the pointer already is sends no event, and the window only
# follows the pointer from its mouse-move messages.
foreach ($step in @(-40, 0, -12, 0)) { $null = [Cursor]::SetCursorPos($x + $step, $y); Start-Sleep -Milliseconds 120 }
Sample "pointer on a node with a hover: transition"

Start-Sleep -Milliseconds 1200
Sample "pointer still there, transition finished"
$null = [Cursor]::SetCursorPos($was.X, $was.Y)
Start-Sleep -Milliseconds 300

if (-not $process.HasExited) { $process.Kill() }
$process.WaitForExit()
# The host writes the rest of its stderr; the client goes first, so it outlives this a moment.
Start-Sleep -Milliseconds 500
Get-Process -Name "gpui-bridge-host" -ErrorAction SilentlyContinue | ForEach-Object { try { $_.Kill() } catch {} }
$lines = $stderr.Result -split "`r?`n" | Where-Object { $_ -match "frames/s" }
Write-Host "--- host frames/s, last 14 lines ---"
$lines | Select-Object -Last 14

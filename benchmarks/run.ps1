# Runs the benches that need numbers from outside the client process: the host's CPU while it
# draws, and its memory while it holds a payload. The two self-measuring benches (tree, micro)
# are run through here too, so one command covers everything.
#
#   pwsh -File benchmarks/run.ps1               # everything
#   pwsh -File benchmarks/run.ps1 -Only render  # tree | micro | render | payload | all
#   pwsh -File benchmarks/run.ps1 -HostPath target/release/gpui-bridge-host.exe   # numbers closer to shipping

param(
  [ValidateSet("all", "tree", "micro", "render", "payload", "images")][string]$Only = "all",
  [string[]]$Images = @(),
  # Host binary for every bench; defaults to the debug build the clients look for in the repository.
  # Use target/release/gpui-bridge-host.exe for numbers closer to what ships.
  [string]$HostPath
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

function Start-Bench([string]$script, [string[]]$arguments = @(), [switch]$Redirect, [string[]]$NodeFlags = @()) {
  $psi = [Diagnostics.ProcessStartInfo]::new()
  $psi.FileName = "node"
  $psi.Arguments = (@($NodeFlags) + @($script) + $arguments) -join " "
  $psi.WorkingDirectory = $root
  $psi.UseShellExecute = $false
  if ($HostPath) { $psi.Environment["GPUI_BRIDGE_HOST"] = (Resolve-Path $HostPath).Path }
  if ($Redirect) {
    $psi.RedirectStandardOutput = $true
  }
  [Diagnostics.Process]::Start($psi)
}

# The host this bench started: the one that was not running before it. (`Get-CimInstance` is not
# usable here: it returns empty rows in this environment, so the parent id is not available.)
function Get-HostProcess([int[]]$Known = @()) {
  Get-Process -Name "gpui-bridge-host" -ErrorAction SilentlyContinue |
    Where-Object { $Known -notcontains $_.Id } |
    Sort-Object StartTime -Descending | Select-Object -First 1
}

function Get-Snapshot([int[]]$Known = @()) {
  $hostProcess = Get-HostProcess -Known $Known
  if (-not $hostProcess) { return $null }
  $hostProcess.Refresh()
  [pscustomobject]@{
    pid  = $hostProcess.Id
    rss  = [math]::Round($hostProcess.WorkingSet64 / 1MB, 1)
    peak = [math]::Round($hostProcess.PeakWorkingSet64 / 1MB, 1)
  }
}

function Get-RunningHostIds {
  @(Get-Process -Name "gpui-bridge-host" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
}

function Invoke-Plain([string]$script, [string[]]$arguments = @(), [string[]]$NodeFlags = @()) {
  Write-Host "`n# node $($NodeFlags -join ' ') $script $($arguments -join ' ')"
  $process = Start-Bench $script $arguments -NodeFlags $NodeFlags
  $process.WaitForExit()
}

switch ($Only) {
  { $_ -in "all", "tree" } { Invoke-Plain "benchmarks/tree.mjs" @() }
  { $_ -in "all", "micro" } { Invoke-Plain "benchmarks/payload-micro.mjs" @() -NodeFlags @("--expose-gc") }
}

if ($Only -in "all", "render") {
  Write-Host "`n# render: host CPU over two 5 s windows (100 % = one core); 200 nodes is the control"
  Write-Host "| nodes | mode | run | rss MB | cpu % first | cpu % second |"
  Write-Host "|---|---|---|---|---|---|"
  foreach ($nodes in 200, 20000, 50000) {
    foreach ($mode in "idle", "spin") {
      foreach ($run in 1, 2) {
        $known = Get-RunningHostIds
        $process = Start-Bench "benchmarks/render.mjs" @($nodes, $mode) -Redirect
        $null = $process.StandardOutput.ReadLine()   # READY
        $hostProcess = Get-HostProcess -Known $known
        if (-not $hostProcess) { Write-Host "| $nodes | $mode | $run | host not found | | |"; $process.Kill(); continue }
        $hostProcess.Refresh()
        $rss = [math]::Round($hostProcess.WorkingSet64 / 1MB, 1)
        $before = $hostProcess.TotalProcessorTime
        Start-Sleep -Seconds 5
        $hostProcess.Refresh()
        $afterFirst = $hostProcess.TotalProcessorTime
        $first = [math]::Round(($afterFirst - $before).TotalSeconds / 5 * 100, 1)
        Start-Sleep -Seconds 5
        $hostProcess.Refresh()
        $second = [math]::Round(($hostProcess.TotalProcessorTime - $afterFirst).TotalSeconds / 5 * 100, 1)
        Write-Host "| $nodes | $mode | $run | $rss | $first | $second |"
        if (-not $process.HasExited) { $process.Kill() }
        $process.WaitForExit()
      }
    }
  }
}

if ($Only -in "all", "payload") {
  Write-Host "`n# payload: one value in one line, host memory after it applied"
  Write-Host "| raw bytes | mode | value chars | client encode ms | rss MB | peak MB | after clear rss MB |"
  Write-Host "|---|---|---|---|---|---|---|"
  foreach ($raw in 65536, 1048576, 4194304) {
    foreach ($mode in "baseline", "retained", "cleared", "path") {
      $known = Get-RunningHostIds
      $process = Start-Bench "benchmarks/payload.mjs" @($raw, $mode) -Redirect
      $applied = $null
      $snapshot = $null
      $afterClear = ""
      while ($null -ne ($line = $process.StandardOutput.ReadLine())) {
        if ($line.StartsWith("APPLIED")) {
          $applied = $line
          Start-Sleep -Milliseconds 400          # let the host finish the commit
          $snapshot = Get-Snapshot -Known $known
        } elseif ($line -eq "CLEARED") {
          Start-Sleep -Milliseconds 400
          $cleared = Get-Snapshot -Known $known
          if ($cleared) { $afterClear = $cleared.rss }
        } elseif ($line -eq "DONE") {
          break
        }
      }
      if (-not $process.HasExited) { $process.Kill() }
      $process.WaitForExit()
      if (-not $applied -or -not $snapshot) { Write-Host "| $raw | $mode | failed | | | | |"; continue }
      $value = [regex]::Match($applied, "value=(\d+)").Groups[1].Value
      $encode = [regex]::Match($applied, "encodeMs=([\d.]+)").Groups[1].Value
      Write-Host "| $raw | $mode | $value | $encode | $($snapshot.rss) | $($snapshot.peak) | $afterClear |"
    }
  }
}

if ($Only -in "all", "images") {
  if (-not $Images.Count) {
    Write-Host "`n# images: skipped (pass -Images <path>... to measure a real image by path)"
  } else {
    Write-Host "`n# images: a real image file shown by path; this is the decode cost, which no transport change moves"
    Write-Host "| file | bytes | rss MB | peak MB |"
    Write-Host "|---|---|---|---|"
    foreach ($image in $Images) {
      $size = (Get-Item $image).Length
      $known = Get-RunningHostIds
      $process = Start-Bench "benchmarks/payload.mjs" @($size, "image", $image) -Redirect
      $snapshot = $null
      while ($null -ne ($line = $process.StandardOutput.ReadLine())) {
        if ($line.StartsWith("APPLIED")) {
          Start-Sleep -Milliseconds 400
          $snapshot = Get-Snapshot -Known $known
        } elseif ($line -eq "DONE") {
          break
        }
      }
      if (-not $process.HasExited) { $process.Kill() }
      $process.WaitForExit()
      $name = Split-Path -Leaf $image
      if ($snapshot) { Write-Host "| $name | $size | $($snapshot.rss) | $($snapshot.peak) |" }
      else { Write-Host "| $name | $size | failed | |" }
    }
  }
}
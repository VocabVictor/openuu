# Performance baseline (docs/perf-baseline-2026-09-13.md): one command that takes every
# measurement of that document which can be taken from a command line, and prints them as
# one table to compare against after a change.
#
#   pwsh -File scripts/perf-baseline.ps1 -Peer <id> -PeerKind vm -PeerSsh user@host
#
# Every measurement says what it is worth: a row is `ok` only when the thing it measures
# was actually exercised. A peer that captured nothing, a log without the diagnostic
# lines, a machine we cannot reach - each comes out as `skipped` with the reason, because
# a baseline that quietly drops a row is worse than one that is short.
param(
  [string]$Peer = '',
  # What the peer can do decides which rows mean anything: a peer with no compositing
  # desktop captures nothing, and one with no GPU encodes in software and is the encoder's
  # bottleneck rather than the link's.
  [ValidateSet('gpu', 'nogpu', 'vm', 'headless', 'unknown')][string]$PeerKind = 'unknown',
  [string]$Exe = "$env:ProgramFiles\OpenUU\OpenUU.exe",
  [string]$PasswordFile = "$HOME\.local\openuu-private\test-peers.txt",
  [string]$PeerSsh = '',
  [int]$Seconds = 60,
  [int]$SettleSeconds = 20,
  [string]$Out = '.\perf-baseline'
)
$ErrorActionPreference = 'Continue'
$ProgressPreference = 'SilentlyContinue'
New-Item -ItemType Directory -Force $Out | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$rows = @()

function Add-Row($section, $metric, $value, $note) {
  $script:rows += [pscustomobject]@{ Section = $section; Metric = $metric; Value = $value; Note = $note }
  # to the host, not the output stream: these functions also return values
  Write-Host ("  {0,-22} {1,-28} {2}" -f $section, $metric, $value)
}
function Skip-Row($section, $metric, $why) { Add-Row $section $metric 'skipped' $why }

function Get-Password {
  if (-not (Test-Path $PasswordFile)) { return $null }
  foreach ($line in Get-Content $PasswordFile) {
    if ($line -match "^\s*peer\s+$([regex]::Escape($Peer))\b.*?permanent\s+(\S+)") { return $Matches[1] }
  }
  return $null
}

# Runs a PowerShell snippet where the peer is, which is this machine when no ssh target
# was given. Returns $null when the peer cannot be reached, so callers can say why.
function Invoke-OnPeer($snippet) {
  if (-not $PeerSsh) {
    if ($Peer) { return $null }   # a remote peer we have no way into
    return (powershell -NoProfile -Command $snippet 2>$null)
  }
  $out = ssh -o BatchMode=yes $PeerSsh "powershell -NoProfile -Command `"$snippet`"" 2>$null
  if ($LASTEXITCODE -ne 0) { return $null }
  return $out
}

# --- 1. connection: how long it took, and whether it went direter than the relay -------
function Measure-Connect {
  $section = 'connection'
  if (-not $Peer) { Skip-Row $section 'establish' 'no -Peer given'; return }
  if (-not (Test-Path $Exe)) { Skip-Row $section 'establish' "no client at $Exe"; return }
  $password = Get-Password
  if (-not $password) { Skip-Row $section 'establish' "no password for $Peer in $PasswordFile"; return }

  $raw = Join-Path $Out "connect.raw.txt"
  $log = Join-Path $Out "controller.log"
  Get-Process openuu -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe } |
    Stop-Process -Force -ErrorAction SilentlyContinue
  $client = Start-Process -FilePath $Exe -ArgumentList @('--connect', $Peer, '--password', $password) `
    -RedirectStandardOutput $raw -RedirectStandardError "$raw.err" -PassThru
  Start-Sleep $Seconds
  # The raw stdout repeats the launch arguments, password included: keep only the lines
  # this script reads, never the whole thing.
  Get-Content $raw -ErrorAction SilentlyContinue |
    Select-String -Pattern 'used to establish|Hole Punched|relay requested|qos_e2e|peer address|Connection Error' |
    Set-Content $log
  Remove-Item $raw, "$raw.err" -Force -ErrorAction SilentlyContinue
  Stop-Process -Id $client.Id -Force -ErrorAction SilentlyContinue
  Get-Process openuu -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe } |
    Stop-Process -Force -ErrorAction SilentlyContinue

  $established = Select-String -Path $log -Pattern 'used to establish (\S+) connection' | Select-Object -First 1
  if (-not $established) { Skip-Row $section 'establish' 'the session never established'; return }
  $transport = $established.Matches[0].Groups[1].Value
  if ($established.Line -match '([\d.]+)(ms|s|µs)\s') {
    $n = [double]$Matches[1]
    $ms = switch ($Matches[2]) { 'ms' { $n } 's' { $n * 1000 } 'µs' { $n / 1000 } }
    Add-Row $section 'establish ms' ([math]::Round($ms, 1)) $transport
  } else {
    Add-Row $section 'establish' $transport 'no time in the line'
  }
  $punched = Select-String -Path $log -Pattern 'Hole Punched' -Quiet
  Add-Row $section 'path' $(if ($punched) { 'direct' } else { 'relay' }) `
    $(if ($punched) { 'hole punched' } else { 'no punch in the log' })
  return $log
}

# --- 2. controller side: the lag a viewer actually sees -------------------------------
function Measure-E2e($log) {
  $section = 'controller'
  if (-not $log -or -not (Test-Path $log)) { Skip-Row $section 'e2e lag' 'no session was run'; return }
  $lines = Select-String -Path $log -Pattern 'qos_e2e .* p50=(\d+) p95=(\d+)'
  if (-not $lines) {
    Skip-Row $section 'e2e lag' 'no qos_e2e lines: set RUSTDESK_QOS_VERBOSE=1 for the controller'; return
  }
  $p50 = $lines | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Sort-Object
  $p95 = $lines | ForEach-Object { [int]$_.Matches[0].Groups[2].Value } | Sort-Object
  Add-Row $section 'e2e p50 median ms' $p50[[int]($p50.Count / 2)] "$($p50.Count) samples"
  Add-Row $section 'e2e p95 median ms' $p95[[int]($p95.Count / 2)] ''
  Add-Row $section 'e2e p95 max ms' ($p95 | Select-Object -Last 1) ''
}

# --- 3. peer side: what the capture loop did ------------------------------------------
function Measure-PeerCapture {
  $section = 'peer capture'
  $log = Invoke-OnPeer 'Get-ChildItem "$env:ProgramData\OpenUU\log\server\*.log","$env:APPDATA\OpenUU\log\*.log" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1 -ExpandProperty FullName'
  if (-not $log) { Skip-Row $section 'captured fps' 'the peer log cannot be reached (give -PeerSsh)'; return }
  $text = Invoke-OnPeer "Select-String -Path '$log' -Pattern 'qos_video' | Select-Object -Last 120 -ExpandProperty Line"
  if (-not $text) { Skip-Row $section 'captured fps' 'no qos_video lines: set RUSTDESK_QOS_VERBOSE=1 on the peer and restart its service'; return }

  $captured = @(); $waits = @()
  foreach ($line in $text) {
    if ($line -match 'captured=(\d+).*wait_max=(\d+)') { $captured += [int]$Matches[1]; $waits += [int]$Matches[2] }
  }
  if (-not $captured.Count) { Skip-Row $section 'captured fps' 'qos_video lines carried no counts'; return }
  $avg = [math]::Round(($captured | Measure-Object -Average).Average, 1)
  Add-Row $section 'captured fps avg' $avg "$($captured.Count) seconds"
  # Section 8 of the baseline: a run where the peer captured nothing measures a still
  # screen, whatever it was meant to measure.
  if ($avg -lt 1) {
    Skip-Row $section 'wait_max ms' 'the peer captured nothing: the fixture was not on the captured desktop'
  } else {
    Add-Row $section 'wait_max ms max' ($waits | Sort-Object | Select-Object -Last 1) ''
  }
}

# --- 4. standby: what the peer does when nobody is connected --------------------------
function Measure-Standby {
  $section = 'standby'
  $cpu = Invoke-OnPeer "(Get-Counter '\Process(openuu*)\% Processor Time' -SampleInterval 5 -MaxSamples 6 -ErrorAction SilentlyContinue).CounterSamples | Measure-Object -Property CookedValue -Sum | Select-Object -ExpandProperty Sum"
  if (-not $cpu) { Skip-Row $section 'cpu percent' 'no counter from the peer (no openuu process, or no way in)'; return }
  $total = ($cpu | Measure-Object -Sum).Sum
  Add-Row $section 'cpu percent (30 s)' ([math]::Round($total / 6, 2)) 'sum over openuu processes'
  $ctx = Invoke-OnPeer "(Get-Counter '\Thread(openuu*)\Context Switches/sec' -SampleInterval 5 -MaxSamples 6 -ErrorAction SilentlyContinue).CounterSamples | Measure-Object -Property CookedValue -Sum | Select-Object -ExpandProperty Sum"
  if (-not $ctx) { Skip-Row $section 'context switches/s' 'the thread counter is not available on the peer'; return }
  Add-Row $section 'context switches/s' ([math]::Round((($ctx | Measure-Object -Sum).Sum) / 6, 0)) 'all openuu threads'
}

# --- 5. teardown: what a session leaves behind ----------------------------------------
function Get-PeerProcesses($tag) {
  $snippet = "Get-Process openuu -ErrorAction SilentlyContinue | Measure-Object -Property Threads.Count,HandleCount,PrivateMemorySize64 -Sum | ForEach-Object { `$_.Sum }"
  $simple = "`$p = Get-Process openuu -ErrorAction SilentlyContinue; if (-not `$p) { '' } else { '{0},{1},{2}' -f ((`$p | ForEach-Object { `$_.Threads.Count } | Measure-Object -Sum).Sum, (`$p | Measure-Object HandleCount -Sum).Sum, [math]::Round(((`$p | Measure-Object PrivateMemorySize64 -Sum).Sum) / 1MB)) }"
  $out = Invoke-OnPeer $simple
  if (-not $out) { return $null }
  $parts = ("$out".Trim() -split ',')
  if ($parts.Count -ne 3) { return $null }
  return [pscustomobject]@{ Tag = $tag; Threads = [int]$parts[0]; Handles = [int]$parts[1]; PrivateMb = [int]$parts[2] }
}

function Measure-Teardown {
  $section = 'teardown'
  $before = Get-PeerProcesses 'before'
  if (-not $before) { Skip-Row $section 'threads before/after' 'the peer processes cannot be listed (give -PeerSsh)'; return }
  if (-not $Peer) { Skip-Row $section 'threads before/after' 'no -Peer given, so no session to end'; return }
  Add-Row $section 'threads before' $before.Threads "handles $($before.Handles), private $($before.PrivateMb) MB"
  Start-Sleep $SettleSeconds
  $after = Get-PeerProcesses 'after'
  if (-not $after) { Skip-Row $section 'threads after' 'the peer stopped answering after the session'; return }
  Add-Row $section 'threads after' $after.Threads "handles $($after.Handles), private $($after.PrivateMb) MB"
  $leaked = ($after.Threads - $before.Threads), ($after.Handles - $before.Handles), ($after.PrivateMb - $before.PrivateMb)
  Add-Row $section 'left behind' ($leaked -join ' / ') 'threads / handles / MB, settled after the session'
}

# --- run ------------------------------------------------------------------------------
Write-Output "perf baseline $stamp, peer '$Peer' ($PeerKind)"
Write-Output ''
$log = Measure-Connect
Measure-E2e $log
Measure-PeerCapture
Measure-Standby
Measure-Teardown

$summary = Join-Path $Out "summary-$stamp.md"
$lines = @(
  "# Performance baseline, $stamp",
  '',
  "Peer ``$Peer`` ($PeerKind), $Seconds s session. Rows marked skipped were not measured;",
  'the note says why.',
  '',
  '| Section | Metric | Value | Note |',
  '| --- | --- | --- | --- |'
)
foreach ($r in $rows) { $lines += "| $($r.Section) | $($r.Metric) | $($r.Value) | $($r.Note) |" }
$lines += ''
$lines += "Taken with ``scripts/perf-baseline.ps1``; compare against the previous run in $Out."
Set-Content -Path $summary -Value $lines -Encoding UTF8
Write-Output ''
Write-Output "wrote $summary"
$skipped = @($rows | Where-Object { $_.Value -eq 'skipped' }).Count
Write-Output "$($rows.Count) rows, $skipped skipped"

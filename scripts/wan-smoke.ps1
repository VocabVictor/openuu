# Cross-WAN smoke test (docs/wan-smoke-plan.md): drives one --connect per case
# and decides it from the client, hbbs and hbbr logs. Read-only on the server;
# the only thing it writes is the controller's per-peer force-always-relay, and
# it restores that from a timestamped backup and verifies the restore.
#
#   pwsh -File scripts/wan-smoke.ps1 -Peer <id> -Case all -Out .\wan-smoke
#
# The password is read from the private peer file and never echoed: the client's
# stdout carries the launch arguments, so only matching log lines are kept.
#
# WHAT THE SAME NETWORK CANNOT DECIDE
# -----------------------------------
# Run from the same egress as the peer, some checks can never pass however healthy
# everything is, because the events they look for are only produced when the two ends are
# actually apart. A green run on one network therefore proves nothing about traversal, and
# nobody should read it that way:
#
#   T1 'hbbs: decision=punch'          hbbs answers decision=local_addr when both ends
#                                      share a network; decision=punch is only reached
#                                      when it has to punch.
#   T1 'hbbs: nat_type not symmetric'  nat_type is logged on the decision=punch line
#                                      alone, so on one network this passes vacuously --
#                                      it is true because nothing was looked at.
#   T1 'client: hole punched'          no hole is punched when the local address works.
#   T3 'hbbs: peer ticket via=response' needs the controlled peer to fail its own punch,
#                                      which needs a real NAT between them.
#
# Measured on 2026-09-13 over a day of a same-network deployment: event=punch_hole 142,
# event=relay_request 73, got paired 66, but decision=punch 0 and event=relay_peer_ticket
# 0. Both of those strings exist in the server source; they were simply never reached.
#
# -NoConnect exercises everything except the sessions (the password lookup, the config
# backup and its verified restore, the server log fetch and whether its event filter still
# matches what the server logs, the report). Use it to check the script itself; it is the
# only part of this file that has ever been run on one network.
#
# STATUS: T1, T2 and T3 have still never been run. Running the script for the first time
# moved it from "would not even start" to "needs the environment"; it did not make it
# verified. Do not read this file as a passed test until someone has run those three
# cases across two networks and said so here.
param(
  [Parameter(Mandatory = $true)][string]$Peer,
  [ValidateSet('T1', 'T2', 'T3', 'all')][string]$Case = 'all',
  [string]$Exe = '',
  [string]$PasswordFile = "$HOME\.local\openuu-private\test-peers.txt",
  [string]$ServerSsh = '',
  [string]$PeerSsh = '',
  [int]$WaitSeconds = 25,
  [string]$Out = '.\wan-smoke',
  # Exercises everything except the sessions: preflight, the password lookup, the
  # force-relay backup and verified restore, the server log fetch and its event filter,
  # and the report. Use it to check the script itself without disturbing a running client.
  [switch]$NoConnect,
  # Kill a client that is already running instead of refusing to start.
  [switch]$Force
)
$ErrorActionPreference = 'Continue'
New-Item -ItemType Directory -Force $Out | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$cfgDir = Join-Path $env:APPDATA 'OpenUU\config'
$peerToml = Join-Path $cfgDir "peers\$Peer.toml"
$results = @()

function Note($t) { Write-Output $t }

# The installer does not always land on C:. Take what was given, else the first candidate
# that exists, else say which ones were tried rather than only naming the default.
function Resolve-Exe([string]$given) {
  if ($given) {
    if (Test-Path $given) { return $given }
    throw "No client at $given"
  }
  $tried = @()
  foreach ($root in @($env:ProgramFiles, ${env:ProgramFiles(x86)}, 'D:\Program Files', 'D:\Program Files (x86)')) {
    if (-not $root) { continue }
    $c = Join-Path $root 'OpenUU\OpenUU.exe'
    $tried += $c
    if (Test-Path $c) { return $c }
  }
  $running = Get-Process openuu -ErrorAction SilentlyContinue |
    Where-Object { $_.Path } | Select-Object -First 1 -ExpandProperty Path
  if ($running) { return $running }
  throw ("No OpenUU client found. Tried: " + ($tried -join '; ') + ". Pass -Exe.")
}

function Get-Password {
  if (-not (Test-Path $PasswordFile)) { throw "No peer file: $PasswordFile" }
  foreach ($line in Get-Content $PasswordFile) {
    if ($line -match "^\s*peer\s+$([regex]::Escape($Peer))\b.*?permanent\s+(\S+)") { return $Matches[1] }
  }
  throw "No permanent password for $Peer in the peer file"
}

# --- per-peer force-always-relay, with a verified restore -------------------
function Set-ForceRelay([bool]$on) {
  if (-not (Test-Path $peerToml)) { Note "  (no peer config yet: $peerToml)"; return $null }
  $backup = "$peerToml.$stamp.bak"
  Copy-Item $peerToml $backup -Force
  $text = Get-Content $peerToml -Raw
  $text = $text -replace "(?m)^\s*force-always-relay\s*=.*\r?\n", ''
  if ($on) {
    if ($text -match "(?m)^\[options\]") { $text = $text -replace "(?m)^\[options\]", "[options]`nforce-always-relay = 'Y'" }
    else { $text = $text.TrimEnd() + "`n`n[options]`nforce-always-relay = 'Y'`n" }
  }
  Set-Content $peerToml $text -NoNewline
  return $backup
}

function Restore-Config($backup) {
  if (-not $backup) { return }
  Copy-Item $backup $peerToml -Force
  $same = (Get-FileHash $backup).Hash -eq (Get-FileHash $peerToml).Hash
  Note ("  restore verified: " + $same)
  if ($same) { Remove-Item $backup -Force }
}

# --- one connection attempt -------------------------------------------------
function Invoke-Connect($tag) {
  $raw = Join-Path $Out "$tag.raw.txt"
  $keep = Join-Path $Out "$tag.client.log"
  # Seconds since the epoch, so neither machine's time zone can widen the window: a
  # wall-clock string is read in the server's local time, and a controller in another zone
  # would ask for hours of unrelated events and could pass on one of them.
  $since = [int][double]::Parse((Get-Date -UFormat %s)) - 5
  $p = Start-Process -FilePath $Exe -ArgumentList @('--connect', $Peer, '--password', (Get-Password)) `
    -RedirectStandardOutput $raw -RedirectStandardError "$raw.err" -PassThru
  Start-Sleep $WaitSeconds
  # the raw stdout repeats the launch arguments, password included: keep only verdict lines
  $kept = Get-Content $raw -ErrorAction SilentlyContinue |
    Select-String -Pattern 'Hole Punched|used to establish|relay requested|rendezvous server|nat type|Connection Error|secure|direct'
  # Written even when empty: a missing file would mean "not looked at", and the checks
  # have to tell that apart from "looked at, not there".
  Set-Content -Path $keep -Value ([string[]]$kept)
  Remove-Item $raw, "$raw.err" -Force -ErrorAction SilentlyContinue
  # Only what this script started: a client the user had open is not ours to close.
  Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
  return @{ Client = $keep; Since = $since }
}

function Get-ServerLog($run, $tag) {
  if (-not $ServerSsh) { return $null }
  $f = Join-Path $Out "$tag.server.log"
  $cmd = "journalctl -u openuu-hbbs -u openuu-hbbr --since '@$($run.Since)' --no-pager | grep -E 'event=punch_hole|event=relay_request|event=relay_response|event=relay_peer_ticket|event=relay_denied|New relay request|got paired'"
  # grep exits 1 when nothing matched, which is not a failure here; only ssh itself failing
  # is. The file is written unconditionally so that its existence means "we looked", which
  # is what the negative checks rely on.
  $lines = ssh -o BatchMode=yes $ServerSsh "$cmd; exit 0" 2>$null
  if ($LASTEXITCODE -ne 0) { Note "  (server log fetch failed: ssh exit $LASTEXITCODE)"; return $null }
  Set-Content -Path $f -Value ([string[]]$lines)
  return $f
}

function Test-Line($file, $pattern) {
  if (-not $file -or -not (Test-Path $file)) { return $false }
  return [bool](Select-String -Path $file -Pattern $pattern -Quiet)
}

# "This must NOT appear" is only a result when the log was actually read. With a plain
# negation, a fetch that never happened reads as proof the line was absent, which turns a
# broken run green -- the wrong direction to be wrong in.
function Test-NoLine($file, $pattern) {
  if (-not $file -or -not (Test-Path $file)) { return $false }
  return -not [bool](Select-String -Path $file -Pattern $pattern -Quiet)
}

function Add-Check($case, $name, $ok, $detail) {
  $script:results += [pscustomobject]@{ Case = $case; Check = $name; Result = $(if ($ok) { 'PASS' } else { 'FAIL' }); Detail = $detail }
  Note ("  [{0}] {1} {2}" -f $(if ($ok) { 'PASS' } else { 'FAIL' }), $name, $detail)
}

# --- cases ------------------------------------------------------------------
function Run-T1 {
  Note '== T1 cross-NAT hole punch'
  $run = Invoke-Connect 'T1'; $srv = Get-ServerLog $run 'T1'
  Add-Check 'T1' 'client: hole punched' (Test-Line $run.Client 'Hole Punched') $run.Client
  Add-Check 'T1' 'client: direct connection' (Test-Line $run.Client 'used to establish Direct') ''
  Add-Check 'T1' 'hbbs: decision=punch' (Test-Line $srv 'decision=punch') $srv
  Add-Check 'T1' 'hbbs: nat_type not symmetric' (Test-NoLine $srv 'decision=punch.*nat_type=SYMMETRIC') ''
  Add-Check 'T1' 'hbbr: no relay paired' (Test-NoLine $srv 'got paired') ''
}

function Run-T2 {
  Note '== T2 symmetric-NAT relay fallback (controller side force-always-relay)'
  $backup = Set-ForceRelay $true
  try {
    $run = Invoke-Connect 'T2'; $srv = Get-ServerLog $run 'T2'
    Add-Check 'T2' 'client: relay requested' (Test-Line $run.Client 'relay requested from peer') $run.Client
    Add-Check 'T2' 'client: not hole punched' (Test-NoLine $run.Client 'Hole Punched') ''
    Add-Check 'T2' 'hbbs: relay_request with uuid' (Test-Line $srv 'event=relay_request .*uuid=') $srv
    Add-Check 'T2' 'hbbr: relay paired' (Test-Line $srv 'got paired') ''
  } finally { Restore-Config $backup }
}

function Run-T3 {
  Note '== T3 peer-initiated relay ticket'
  if ($PeerSsh) {
    Note "  asking the controlled peer to force relay via $PeerSsh"
    ssh -o BatchMode=yes $PeerSsh "powershell -NoProfile -Command `"`$p='C:\Windows\ServiceProfiles\LocalService\AppData\Roaming\OpenUU\config\peers'; Get-ChildItem `$p -Filter '*.toml' | ForEach-Object { `$_.FullName }`"" 2>$null | ForEach-Object { Note "  peer config: $_" }
    Note '  set force-always-relay there by hand before this case (the script does not write on the peer)'
  } else {
    Note '  no -PeerSsh: this case needs the controlled peer to fail its own punch (symmetric NAT or force-always-relay there)'
  }
  $run = Invoke-Connect 'T3'; $srv = Get-ServerLog $run 'T3'
  Add-Check 'T3' 'hbbs: relay_response forwarded' (Test-Line $srv 'event=relay_response .*refuse=false') $srv
  Add-Check 'T3' 'hbbs: peer ticket via response' (Test-Line $srv 'event=relay_peer_ticket .*via=response') ''
  Add-Check 'T3' 'hbbr: relay paired' (Test-Line $srv 'got paired') ''
  Add-Check 'T3' 'hbbr: ticket not denied' (Test-NoLine $srv 'event=relay_denied') ''
}

# --- preflight --------------------------------------------------------------
Note "== preflight"
$Exe = Resolve-Exe $Exe
$egress = try { (Invoke-WebRequest -UseBasicParsing -TimeoutSec 8 'https://ifconfig.me/ip').Content.Trim() } catch { 'unknown' }
Note "  controller public egress: $egress"
Note "  peer: $Peer   exe: $Exe"
$null = Get-Password   # fail here rather than half way through a case
Note "  password for ${Peer}: found"
if (-not $ServerSsh) { Note '  no -ServerSsh: server-side checks will be skipped, join the logs by hand' }
Note '  the peer must sit behind a DIFFERENT egress; stop here if it does not'

# A client that is already running would answer the connect, steal the session or simply
# confuse the logs. Say so instead of closing somebody's window.
$live = @(Get-Process openuu -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe })
if ($live.Count -gt 0 -and -not $NoConnect) {
  if ($Force) {
    Note "  stopping $($live.Count) running client process(es) because -Force was given"
    $live | Stop-Process -Force -ErrorAction SilentlyContinue
  }
  else {
    throw ("An OpenUU client is already running (pid " + (($live | ForEach-Object { $_.Id }) -join ', ') +
      "). Close it, or pass -Force to stop it, or -NoConnect to check the script without sessions.")
  }
}

if ($NoConnect) {
  Note '== NoConnect: the sessions are skipped; everything around them is exercised'
  $backup = Set-ForceRelay $true
  Add-Check 'dry' 'config: force-always-relay written' `
    (Test-Line $peerToml "force-always-relay") $peerToml
  Restore-Config $backup
  Add-Check 'dry' 'config: restore verified and backup removed' `
    ((-not $backup) -or -not (Test-Path $backup)) ''
  # A day wide on purpose. The point here is not what happened lately but whether the
  # event filter still matches the shape the server actually logs: a pattern that no
  # longer matches would make every server-side check fail during a real run, and it would
  # look like the connection failed rather than like the filter did.
  $run = @{ Client = $null; Since = ([int][double]::Parse((Get-Date -UFormat %s)) - 86400) }
  $srv = Get-ServerLog $run 'dry'
  Add-Check 'dry' 'server: log reachable' ($null -ne $srv -and (Test-Path $srv)) ([string]$srv)
  $lines = @(Get-Content $srv -ErrorAction SilentlyContinue)
  if ($lines.Count -gt 0) {
    Add-Check 'dry' 'server: event filter matches the real log format' $true `
      ("$($lines.Count) line(s) in the last day")
  }
  else {
    Note '  [SKIP] server: event filter matches the real log format -- no session in the last day to match against'
  }
}
else {
  switch ($Case) {
    'T1' { Run-T1 }
    'T2' { Run-T2 }
    'T3' { Run-T3 }
    'all' { Run-T1; Run-T2; Run-T3 }
  }
}

$report = Join-Path $Out "report-$stamp.csv"
$results | Export-Csv $report -NoTypeInformation -Encoding UTF8
Note ''
$results | Format-Table -AutoSize
Note "report: $report"
if ($results.Result -contains 'FAIL') { exit 1 }

# Cross-WAN smoke test (docs/wan-smoke-plan.md): drives one --connect per case
# and decides it from the client, hbbs and hbbr logs. Read-only on the server;
# the only thing it writes is the controller's per-peer force-always-relay, and
# it restores that from a timestamped backup and verifies the restore.
#
#   pwsh -File scripts/wan-smoke.ps1 -Peer <id> -Case all -Out .\wan-smoke
#
# The password is read from the private peer file and never echoed: the client's
# stdout carries the launch arguments, so only matching log lines are kept.
param(
  [Parameter(Mandatory = $true)][string]$Peer,
  [ValidateSet('T1', 'T2', 'T3', 'all')][string]$Case = 'all',
  [string]$Exe = "$env:ProgramFiles\OpenUU\OpenUU.exe",
  [string]$PasswordFile = "$HOME\.local\openuu-private\test-peers.txt",
  [string]$ServerSsh = '',
  [string]$PeerSsh = '',
  [int]$WaitSeconds = 25,
  [string]$Out = '.\wan-smoke'
)
$ErrorActionPreference = 'Continue'
New-Item -ItemType Directory -Force $Out | Out-Null
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$cfgDir = Join-Path $env:APPDATA 'OpenUU\config'
$peerToml = Join-Path $cfgDir "peers\$Peer.toml"
$results = @()

function Note($t) { Write-Output $t }

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
  $since = (Get-Date).ToUniversalTime().AddSeconds(-5).ToString('yyyy-MM-dd HH:mm:ss')
  Get-Process openuu -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe } | Stop-Process -Force -ErrorAction SilentlyContinue
  $p = Start-Process -FilePath $Exe -ArgumentList @('--connect', $Peer, '--password', (Get-Password)) `
    -RedirectStandardOutput $raw -RedirectStandardError "$raw.err" -PassThru
  Start-Sleep $WaitSeconds
  # the raw stdout repeats the launch arguments, password included: keep only verdict lines
  Get-Content $raw -ErrorAction SilentlyContinue |
    Select-String -Pattern 'Hole Punched|used to establish|relay requested|rendezvous server|nat type|Connection Error|secure|direct' |
    Set-Content $keep
  Remove-Item $raw, "$raw.err" -Force -ErrorAction SilentlyContinue
  Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
  Get-Process openuu -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe } | Stop-Process -Force -ErrorAction SilentlyContinue
  return @{ Client = $keep; Since = $since }
}

function Get-ServerLog($run, $tag) {
  if (-not $ServerSsh) { return $null }
  $f = Join-Path $Out "$tag.server.log"
  $cmd = "journalctl -u openuu-hbbs -u openuu-hbbr --since '$($run.Since)' --no-pager | grep -E 'event=punch_hole|event=relay_request|event=relay_response|event=relay_peer_ticket|event=relay_denied|New relay request|got paired'"
  ssh -o BatchMode=yes $ServerSsh $cmd 2>$null | Set-Content $f
  return $f
}

function Test-Line($file, $pattern) {
  if (-not $file -or -not (Test-Path $file)) { return $false }
  return [bool](Select-String -Path $file -Pattern $pattern -Quiet)
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
  Add-Check 'T1' 'hbbs: nat_type not symmetric' (-not (Test-Line $srv 'decision=punch.*nat_type=SYMMETRIC')) ''
  Add-Check 'T1' 'hbbr: no relay paired' (-not (Test-Line $srv 'got paired')) ''
}

function Run-T2 {
  Note '== T2 symmetric-NAT relay fallback (controller side force-always-relay)'
  $backup = Set-ForceRelay $true
  try {
    $run = Invoke-Connect 'T2'; $srv = Get-ServerLog $run 'T2'
    Add-Check 'T2' 'client: relay requested' (Test-Line $run.Client 'relay requested from peer') $run.Client
    Add-Check 'T2' 'client: not hole punched' (-not (Test-Line $run.Client 'Hole Punched')) ''
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
  Add-Check 'T3' 'hbbr: ticket not denied' (-not (Test-Line $srv 'event=relay_denied')) ''
}

# --- preflight --------------------------------------------------------------
Note "== preflight"
$egress = try { (Invoke-WebRequest -UseBasicParsing -TimeoutSec 8 'https://ifconfig.me/ip').Content.Trim() } catch { 'unknown' }
Note "  controller public egress: $egress"
Note "  peer: $Peer   exe: $Exe"
if (-not (Test-Path $Exe)) { throw "No client at $Exe" }
if (-not $ServerSsh) { Note '  no -ServerSsh: server-side checks will be skipped, join the logs by hand' }
Note '  the peer must sit behind a DIFFERENT egress; stop here if it does not'

switch ($Case) {
  'T1' { Run-T1 }
  'T2' { Run-T2 }
  'T3' { Run-T3 }
  'all' { Run-T1; Run-T2; Run-T3 }
}

$report = Join-Path $Out "report-$stamp.csv"
$results | Export-Csv $report -NoTypeInformation -Encoding UTF8
Note ''
$results | Format-Table -AutoSize
Note "report: $report"
if ($results.Result -contains 'FAIL') { exit 1 }

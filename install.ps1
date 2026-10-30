# KIQR installer for Windows — https://kiqr.dev/install.ps1
#
#   irm https://kiqr.dev/install.ps1 | iex
#
# Downloads the latest KIQR release for this machine, checks its SHA-256,
# puts kiqr.exe on your PATH and starts the daemon at logon. Run it again to
# update; it says when you already have the latest.
#
#   $env:KIQR_VERSION = "v0.3.0"   install that release instead of the latest (pre-releases too)
#   $env:KIQR_BIN_DIR = "C:\tools" where kiqr.exe goes (default %LOCALAPPDATA%\kiqr\bin)
#   $env:KIQR_NO_SERVICE = "1"     don't register the daemon; start it with `kiqr start`
#   $env:KIQR_RELEASES = "<url>"   a mirror of github.com/kiqr/kiqr/releases

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$Releases = if ($env:KIQR_RELEASES) { $env:KIQR_RELEASES } else { "https://github.com/kiqr/kiqr/releases" }
$Version = if ($env:KIQR_VERSION) { $env:KIQR_VERSION } else { "latest" }
$BinDir = if ($env:KIQR_BIN_DIR) { $env:KIQR_BIN_DIR } else { Join-Path $env:LOCALAPPDATA "kiqr\bin" }
$Exe = Join-Path $BinDir "kiqr.exe"
$Task = "KIQR daemon"

function Fail($msg) { Write-Host "kiqr install: $msg" -ForegroundColor Red; exit 1 }

function Get-KiqrVersion($path) {
  if (-not (Test-Path $path)) { return $null }
  try { ((& $path --version) -split " ")[1] } catch { $null }
}

$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
  "AMD64" { "x86_64" }
  "ARM64" { "aarch64" }
  default { Fail "there is no prebuilt KIQR for $($env:PROCESSOR_ARCHITECTURE) Windows." }
}
$target = "$arch-pc-windows-msvc"

if ($Version -eq "latest") {
  $base = "$Releases/latest/download"
  $archive = "kiqr-$target.zip"
} else {
  $v = "v" + $Version.TrimStart("v")
  $base = "$Releases/download/$v"
  $archive = "kiqr-$v-$target.zip"
}

$before = Get-KiqrVersion $Exe
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("kiqr-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  Write-Host "Downloading KIQR ($Version) for $target..."
  try {
    Invoke-WebRequest -UseBasicParsing -Uri "$base/$archive" -OutFile (Join-Path $tmp $archive)
    Invoke-WebRequest -UseBasicParsing -Uri "$base/SHA256SUMS" -OutFile (Join-Path $tmp "SHA256SUMS")
  } catch { Fail "could not download $base/$archive ($($_.Exception.Message))" }

  $want = (Get-Content (Join-Path $tmp "SHA256SUMS") | Where-Object { ($_ -split "\s+")[1] -eq $archive } | ForEach-Object { ($_ -split "\s+")[0] }) | Select-Object -First 1
  if (-not $want) { Fail "$archive is not in the release's SHA256SUMS" }
  $got = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $archive)).Hash.ToLower()
  if ($got -ne $want.ToLower()) { Fail "checksum mismatch for $archive (expected $want, got $got)" }

  Expand-Archive -Path (Join-Path $tmp $archive) -DestinationPath $tmp -Force
  $bin = Get-ChildItem -Path $tmp -Recurse -Filter "kiqr.exe" | Select-Object -First 1
  if (-not $bin) { Fail "the archive holds no kiqr.exe" }
  $new = Get-KiqrVersion $bin.FullName
  if (-not $new) { Fail "the downloaded kiqr.exe does not run on this machine" }

  if ($new -eq $before) {
    Write-Host "KIQR $new is already the latest."
    exit 0
  }

  # A running kiqr.exe cannot be overwritten: stop the daemon first, or move the old one aside.
  if (-not $env:KIQR_NO_SERVICE) { Stop-ScheduledTask -TaskName $Task -ErrorAction SilentlyContinue }
  Get-Process -Name kiqr -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $Exe } | Stop-Process -Force -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Path $BinDir -Force | Out-Null
  if (Test-Path $Exe) {
    $old = "$Exe.old"
    Remove-Item $old -Force -ErrorAction SilentlyContinue
    Move-Item $Exe $old -Force
  }
  Copy-Item $bin.FullName $Exe -Force
  Remove-Item "$Exe.old" -Force -ErrorAction SilentlyContinue
} finally {
  Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}

# On the user's PATH, for new terminals and this one.
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (-not (($userPath -split ";") -contains $BinDir)) {
  [Environment]::SetEnvironmentVariable("Path", ($(if ($userPath) { "$userPath;" } else { "" }) + $BinDir), "User")
  Write-Host "Added $BinDir to your PATH (open a new terminal to use it)."
}
if (-not (($env:Path -split ";") -contains $BinDir)) { $env:Path = "$env:Path;$BinDir" }

# The daemon starts at logon, hidden, and restarts if it stops.
if (-not $env:KIQR_NO_SERVICE) {
  $action = New-ScheduledTaskAction -Execute $Exe -Argument "daemon"
  $trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
  # Parallel: an in-app update starts the new version as a second instance of this task, then the old one exits.
  $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -MultipleInstances Parallel -Hidden
  Register-ScheduledTask -TaskName $Task -Action $action -Trigger $trigger -Settings $settings -Description "KIQR daemon" -Force | Out-Null
  Start-ScheduledTask -TaskName $Task
  # Wait for it to answer.
  for ($i = 0; $i -lt 30; $i++) {
    & $Exe status *> $null
    if ($LASTEXITCODE -eq 0) { (& $Exe status) | Select-Object -First 2 | ForEach-Object { Write-Host $_ }; break }
    Start-Sleep -Seconds 1
  }
}

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
  Write-Host "note: KIQR keeps your data in a git repository; install Git for Windows before you start (winget install Git.Git)."
}

$after = Get-KiqrVersion $Exe
Write-Host ""
if ($before) {
  Write-Host "Updated KIQR $before -> $after."
} else {
  Write-Host "KIQR $after is installed and starts at logon. Open http://127.0.0.1:4040 or run:"
  Write-Host "  kiqr project add https://your-site.com --watch"
}
Write-Host "Manage it with: kiqr status | start | stop | restart | update"

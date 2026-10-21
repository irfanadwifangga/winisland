<#
.SYNOPSIS
  Doctor: cek kesiapan lingkungan untuk proyek winisland (Rust + Slint di Windows).

.DESCRIPTION
  Script ini hanya MEMERIKSA. Tidak memasang atau mengubah apa pun.
  Setiap temuan WARN/FAIL disertai perintah perbaikan yang disarankan di ringkasan akhir.
  Exit code: 0 jika tidak ada FAIL, 1 jika ada FAIL (berguna untuk otomasi).

.PARAMETER Check
  Jalankan juga `cargo check` pada proyek di folder saat ini.

.PARAMETER NoNetwork
  Lewati tes koneksi ke crates.io.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\setup.ps1

.EXAMPLE
  .\setup.ps1 -Check
#>
[CmdletBinding()]
param(
    [switch]$Check,
    [switch]$NoNetwork
)

$ErrorActionPreference = 'Continue'

$script:Pass = 0
$script:Warn = 0
$script:Fail = 0
$script:Fixes = New-Object System.Collections.Generic.List[string]

function Write-Result {
    param(
        [ValidateSet('OK', 'WARN', 'FAIL', 'INFO')][string]$Status,
        [string]$Name,
        [string]$Detail = '',
        [string]$Fix = ''
    )
    switch ($Status) {
        'OK'   { $color = 'Green';  $script:Pass++ }
        'WARN' { $color = 'Yellow'; $script:Warn++ }
        'FAIL' { $color = 'Red';    $script:Fail++ }
        'INFO' { $color = 'Cyan' }
    }
    $line = '[{0,-4}] {1}' -f $Status, $Name
    if ($Detail) { $line += ' - ' + $Detail }
    Write-Host $line -ForegroundColor $color
    if ($Fix -and ($Status -eq 'WARN' -or $Status -eq 'FAIL')) {
        $script:Fixes.Add(('{0}: {1}' -f $Name, $Fix))
    }
}

function Write-Section {
    param([string]$Title)
    Write-Host ''
    Write-Host "== $Title ==" -ForegroundColor White
}

function Test-Cmd {
    param([string]$Name)
    return [bool](Get-Command $Name -ErrorAction SilentlyContinue)
}

function Test-Tcp {
    param([string]$HostName, [int]$Port, [int]$TimeoutMs = 3000)
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $iar = $client.BeginConnect($HostName, $Port, $null, $null)
        if (-not $iar.AsyncWaitHandle.WaitOne($TimeoutMs, $false)) { return $false }
        $client.EndConnect($iar)
        return $true
    }
    catch { return $false }
    finally { $client.Close() }
}

Write-Host ''
Write-Host 'winisland doctor - cek kesiapan lingkungan (hanya membaca, tidak mengubah apa pun)' -ForegroundColor White

if ($env:OS -ne 'Windows_NT') {
    Write-Result -Status FAIL -Name 'Sistem operasi' -Detail 'Script ini hanya untuk Windows.'
    exit 1
}

# Arsitektur (untuk memilih komponen MSVC dan folder SDK yang tepat)
$archRaw = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
$isArm = ($archRaw -eq 'ARM64')
$libArch = if ($isArm) { 'arm64' } else { 'x64' }
$vcComponent = if ($isArm) { 'Microsoft.VisualStudio.Component.VC.Tools.ARM64' } else { 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' }

# ---------------------------------------------------------------- Sistem
Write-Section 'Sistem'

$os = Get-CimInstance Win32_OperatingSystem -ErrorAction SilentlyContinue
if ($os) {
    $build = [int]$os.BuildNumber
    $osName = if ($build -ge 22000) { 'Windows 11' } else { 'Windows 10' }
    if ($build -ge 19041) {
        Write-Result -Status OK -Name 'Versi Windows' -Detail ("{0} (build {1}, {2})" -f $osName, $build, $archRaw)
    }
    else {
        Write-Result -Status WARN -Name 'Versi Windows' -Detail ("{0} build {1} tergolong lama" -f $osName, $build) -Fix 'Perbarui Windows ke 10 versi 2004 atau lebih baru (disarankan Windows 11).'
    }
}
else {
    Write-Result -Status WARN -Name 'Versi Windows' -Detail 'Tidak bisa membaca informasi OS.'
}

$psv = $PSVersionTable.PSVersion
if ($psv.Major -ge 5) {
    Write-Result -Status OK -Name 'PowerShell' -Detail $psv.ToString()
}
else {
    Write-Result -Status WARN -Name 'PowerShell' -Detail $psv.ToString() -Fix 'Gunakan PowerShell 5.1 atau lebih baru.'
}

# Ruang disk di drive tempat folder saat ini berada
$drive = (Get-Location).Drive
if ($drive -and $null -ne $drive.Free) {
    $freeGb = [math]::Round($drive.Free / 1GB, 1)
    if ($freeGb -ge 10) {
        Write-Result -Status OK -Name 'Ruang disk' -Detail ("{0} GB bebas di {1}:" -f $freeGb, $drive.Name)
    }
    else {
        Write-Result -Status WARN -Name 'Ruang disk' -Detail ("hanya {0} GB bebas di {1}:" -f $freeGb, $drive.Name) -Fix 'Sediakan minimal ~10 GB (folder target/ Rust dan Build Tools cukup besar).'
    }
}

if (-not $NoNetwork) {
    if (Test-Tcp -HostName 'index.crates.io' -Port 443) {
        Write-Result -Status OK -Name 'Koneksi crates.io' -Detail 'index.crates.io:443 terjangkau'
    }
    else {
        Write-Result -Status WARN -Name 'Koneksi crates.io' -Detail 'index.crates.io:443 tidak terjangkau' -Fix 'Periksa internet/proxy/firewall; cargo butuh akses ini untuk mengunduh crate.'
    }
}

# ---------------------------------------------------------------- Alat umum
Write-Section 'Alat umum'

if (Test-Cmd 'winget') {
    Write-Result -Status OK -Name 'winget' -Detail ((& winget --version) -join ' ')
}
else {
    Write-Result -Status WARN -Name 'winget' -Detail 'tidak ditemukan (hanya memudahkan instalasi)' -Fix 'Pasang "App Installer" dari Microsoft Store, atau unduh installer secara manual.'
}

if (Test-Cmd 'git') {
    Write-Result -Status OK -Name 'git' -Detail ((& git --version) -join ' ')
}
else {
    Write-Result -Status WARN -Name 'git' -Detail 'tidak ditemukan' -Fix 'winget install --id Git.Git'
}

# ---------------------------------------------------------------- Build tools (MSVC)
Write-Section 'C++ Build Tools (linker MSVC)'

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vsFix = 'winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"  (atau pasang manual workload "Desktop development with C++")'

if (Test-Path $vswhere) {
    $vsPath = (& $vswhere -latest -products '*' -requires $vcComponent -property installationPath) | Select-Object -First 1
    if ($vsPath) {
        $vsName = (& $vswhere -latest -products '*' -requires $vcComponent -property displayName) | Select-Object -First 1
        Write-Result -Status OK -Name 'MSVC C++ tools' -Detail ("{0}" -f $vsName)
    }
    else {
        Write-Result -Status FAIL -Name 'MSVC C++ tools' -Detail 'Visual Studio ada, tetapi workload C++ (MSVC) belum terpasang' -Fix $vsFix
    }
}
else {
    Write-Result -Status FAIL -Name 'MSVC C++ tools' -Detail 'Visual Studio / Build Tools tidak ditemukan' -Fix $vsFix
}

$kitsLib = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Lib'
$sdk = $null
if (Test-Path $kitsLib) {
    $sdk = Get-ChildItem $kitsLib -Directory -ErrorAction SilentlyContinue |
        Where-Object { Test-Path (Join-Path $_.FullName ("um\{0}\kernel32.lib" -f $libArch)) } |
        Sort-Object Name -Descending |
        Select-Object -First 1
}
if ($sdk) {
    Write-Result -Status OK -Name 'Windows SDK' -Detail ("versi {0}" -f $sdk.Name)
}
else {
    Write-Result -Status FAIL -Name 'Windows SDK' -Detail 'library SDK (kernel32.lib) tidak ditemukan' -Fix 'Tambahkan komponen "Windows 11 SDK" (atau Windows 10 SDK) lewat Visual Studio Installer.'
}

# ---------------------------------------------------------------- Rust
Write-Section 'Rust toolchain'

$rustupInPath = Test-Cmd 'rustup'
if (-not $rustupInPath) {
    $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin\rustup.exe'
    if (Test-Path $cargoBin) {
        Write-Result -Status FAIL -Name 'rustup' -Detail 'terpasang tetapi belum ada di PATH' -Fix 'Buka terminal BARU (PATH belum ter-refresh), atau tambahkan %USERPROFILE%\.cargo\bin ke PATH.'
    }
    else {
        Write-Result -Status FAIL -Name 'rustup' -Detail 'belum terpasang' -Fix 'winget install --id Rustlang.Rustup   (lalu buka terminal baru)'
    }
}
else {
    Write-Result -Status OK -Name 'rustup' -Detail ((& rustup --version 2>$null | Select-Object -First 1) -join ' ')

    if (Test-Cmd 'rustc') {
        Write-Result -Status OK -Name 'rustc' -Detail ((& rustc --version) -join ' ')
    }
    else {
        Write-Result -Status FAIL -Name 'rustc' -Detail 'tidak ditemukan' -Fix 'rustup default stable-msvc'
    }

    if (Test-Cmd 'cargo') {
        Write-Result -Status OK -Name 'cargo' -Detail ((& cargo --version) -join ' ')
    }
    else {
        Write-Result -Status FAIL -Name 'cargo' -Detail 'tidak ditemukan' -Fix 'rustup default stable-msvc'
    }

    if (Test-Cmd 'rustc') {
        $vv = & rustc -vV 2>$null
        $hostLine = $vv | Select-String -Pattern '^host:\s*(\S+)' | Select-Object -First 1
        $hostTriple = if ($hostLine) { $hostLine.Matches[0].Groups[1].Value } else { '' }
        if ($hostTriple -match 'windows-msvc$') {
            Write-Result -Status OK -Name 'Target host' -Detail $hostTriple
        }
        elseif ($hostTriple) {
            Write-Result -Status FAIL -Name 'Target host' -Detail ("{0} (bukan MSVC)" -f $hostTriple) -Fix 'rustup default stable-msvc'
        }
        else {
            Write-Result -Status WARN -Name 'Target host' -Detail 'tidak bisa dibaca dari rustc -vV'
        }

        $active = (& rustup show active-toolchain 2>$null | Select-Object -First 1)
        if ($active) { Write-Result -Status INFO -Name 'Toolchain aktif' -Detail ($active -join ' ') }

        $components = & rustup component list --installed 2>$null
        foreach ($c in @('clippy', 'rustfmt')) {
            if ($components -match ("^{0}" -f $c)) {
                Write-Result -Status OK -Name ("Komponen {0}" -f $c)
            }
            else {
                Write-Result -Status WARN -Name ("Komponen {0}" -f $c) -Detail 'belum terpasang' -Fix 'rustup component add clippy rustfmt'
            }
        }
    }
}

# ---------------------------------------------------------------- Editor
Write-Section 'Editor (opsional)'

if (Test-Cmd 'code') {
    Write-Result -Status OK -Name 'VS Code' -Detail 'perintah "code" tersedia'
    $exts = & code --list-extensions 2>$null
    if ($exts -contains 'rust-lang.rust-analyzer') {
        Write-Result -Status OK -Name 'rust-analyzer' -Detail 'ekstensi terpasang'
    }
    else {
        Write-Result -Status WARN -Name 'rust-analyzer' -Detail 'ekstensi belum terpasang' -Fix 'code --install-extension rust-lang.rust-analyzer'
    }
}
else {
    Write-Result -Status WARN -Name 'VS Code' -Detail 'perintah "code" tidak ditemukan (lewati jika memakai editor lain)' -Fix 'winget install --id Microsoft.VisualStudioCode'
}

# ---------------------------------------------------------------- Proyek
Write-Section 'Proyek (folder saat ini)'

if (Test-Path 'Cargo.toml') {
    Write-Result -Status OK -Name 'Cargo.toml' -Detail 'ditemukan'

    $pkgMatch = Select-String -Path 'Cargo.toml' -Pattern '^\s*name\s*=\s*"([^"]+)"' | Select-Object -First 1
    $pkg = if ($pkgMatch) { $pkgMatch.Matches[0].Groups[1].Value } else { '' }
    if ($pkg) { Write-Result -Status INFO -Name 'Nama paket' -Detail $pkg }

    $cargoText = Get-Content 'Cargo.toml' -Raw
    foreach ($dep in @(
            @{ Pattern = '(?m)^\s*slint\s*='; Label = 'Dependensi slint'; Fix = 'cargo add slint' },
            @{ Pattern = '(?m)^\s*slint-build\s*='; Label = 'Build-dependency slint-build'; Fix = 'cargo add --build slint-build' }
        )) {
        if ($cargoText -match $dep.Pattern) {
            Write-Result -Status OK -Name $dep.Label
        }
        else {
            Write-Result -Status WARN -Name $dep.Label -Detail 'belum ada di Cargo.toml' -Fix $dep.Fix
        }
    }

    if ($cargoText -match '(?m)^\s*build\s*=\s*"build\.rs"') {
        Write-Result -Status OK -Name 'Cargo.toml: build = "build.rs"'
    }
    else {
        Write-Result -Status WARN -Name 'Cargo.toml: build = "build.rs"' -Detail 'belum diatur di [package]' -Fix 'Tambahkan baris build = "build.rs" di bagian [package] (lihat rencana).'
    }

    foreach ($f in @('build.rs', 'src\main.rs', 'ui\main.slint')) {
        if (Test-Path $f) {
            Write-Result -Status OK -Name $f
        }
        else {
            Write-Result -Status WARN -Name $f -Detail 'tidak ditemukan' -Fix ("Buat file {0} sesuai panduan di rencana." -f $f)
        }
    }

    if ($pkg) {
        $exe = Join-Path 'target\release' ($pkg + '.exe')
        if (Test-Path $exe) {
            $sizeMb = [math]::Round((Get-Item $exe).Length / 1MB, 2)
            Write-Result -Status INFO -Name 'Ukuran exe release' -Detail ("{0} MB ({1})" -f $sizeMb, $exe)
        }
        else {
            Write-Result -Status INFO -Name 'Ukuran exe release' -Detail 'belum ada build release (jalankan: cargo build --release)'
        }
    }

    if ($Check) {
        if (Test-Cmd 'cargo') {
            Write-Host ''
            Write-Host 'Menjalankan cargo check ...' -ForegroundColor Cyan
            $sw = [System.Diagnostics.Stopwatch]::StartNew()
            & cargo check
            $code = $LASTEXITCODE
            $sw.Stop()
            if ($code -eq 0) {
                Write-Result -Status OK -Name 'cargo check' -Detail ("lolos dalam {0:N1} dtk" -f $sw.Elapsed.TotalSeconds)
            }
            else {
                Write-Result -Status FAIL -Name 'cargo check' -Detail ("gagal (exit {0}); lihat pesan error di atas" -f $code) -Fix 'Perbaiki error kompilasi yang ditampilkan di atas.'
            }
        }
        else {
            Write-Result -Status WARN -Name 'cargo check' -Detail 'dilewati karena cargo tidak tersedia'
        }
    }
    else {
        Write-Result -Status INFO -Name 'cargo check' -Detail 'dilewati (jalankan dengan -Check untuk menguji kompilasi)'
    }
}
else {
    Write-Result -Status INFO -Name 'Proyek' -Detail 'Cargo.toml tidak ada di folder ini. Bagian proyek dilewati.'
    Write-Host '       Buat proyek dengan: cargo new winisland' -ForegroundColor DarkGray
}

# ---------------------------------------------------------------- Ringkasan
Write-Host ''
Write-Host '== Ringkasan ==' -ForegroundColor White
Write-Host ("OK: {0}   WARN: {1}   FAIL: {2}" -f $script:Pass, $script:Warn, $script:Fail)

if ($script:Fixes.Count -gt 0) {
    Write-Host ''
    Write-Host 'Perbaikan yang disarankan:' -ForegroundColor Yellow
    $i = 1
    foreach ($fix in $script:Fixes) {
        Write-Host ("  {0}. {1}" -f $i, $fix)
        $i++
    }
}

Write-Host ''
if ($script:Fail -eq 0) {
    Write-Host 'Siap. Tidak ada FAIL.' -ForegroundColor Green
    exit 0
}
else {
    Write-Host 'Belum siap. Perbaiki item FAIL lalu jalankan ulang script ini.' -ForegroundColor Red
    exit 1
}

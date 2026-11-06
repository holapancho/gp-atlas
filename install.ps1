# GP Atlas installer for Windows (PowerShell 5.1 or 7).
#
#   irm https://raw.githubusercontent.com/holapancho/gp-atlas/main/install.ps1 | iex
#
# Downloads the Windows release zip from GitHub Releases, checks its SHA-256
# against the release's SHA256SUMS, copies gp-atlas.exe and gp-atlas-beta.exe
# into $env:GP_ATLAS_INSTALL_DIR (default: %LOCALAPPDATA%\gp-atlas\bin) and
# adds that folder to the user PATH. No administrator rights needed.
#
# Environment: GP_ATLAS_VERSION (a tag such as v0.1.0; default latest),
# GP_ATLAS_INSTALL_DIR, GP_ATLAS_DOWNLOAD_URL (base URL, for testing mirrors).

# Runs in its own scope so `irm | iex` leaves the caller's session settings alone.
& {
    $ErrorActionPreference = 'Stop'
    $Repo = 'holapancho/gp-atlas'
    $Version = if ($env:GP_ATLAS_VERSION) { $env:GP_ATLAS_VERSION } else { 'latest' }
    $InstallDir = if ($env:GP_ATLAS_INSTALL_DIR) { $env:GP_ATLAS_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA 'gp-atlas\bin' }

    if ($Version -eq 'latest') {
        $Base = "https://github.com/$Repo/releases/latest/download"
    } elseif ($Version -match '^v[0-9]') {
        $Base = "https://github.com/$Repo/releases/download/$Version"
    } else {
        throw "GP_ATLAS_VERSION must look like v0.1.0 (got '$Version')"
    }
    if ($env:GP_ATLAS_DOWNLOAD_URL) { $Base = $env:GP_ATLAS_DOWNLOAD_URL }

    if (-not [Environment]::Is64BitOperatingSystem) { throw 'GP Atlas needs 64-bit Windows.' }
    $Asset = 'gp-atlas-x86_64-pc-windows-msvc.zip'

    $Tmp = Join-Path ([IO.Path]::GetTempPath()) ("gp-atlas-" + [Guid]::NewGuid())
    New-Item -ItemType Directory -Path $Tmp | Out-Null
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        Write-Host "gp-atlas install: downloading $Asset ($Version)"
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$Base/$Asset" -OutFile (Join-Path $Tmp $Asset)
        } catch {
            throw "Could not download $Base/$Asset. If no release is published yet, build from source: cargo install --locked --git https://github.com/$Repo gp-atlas-tui gp-atlas-cli"
        }
        Invoke-WebRequest -UseBasicParsing -Uri "$Base/SHA256SUMS" -OutFile (Join-Path $Tmp 'SHA256SUMS')

        $Line = Get-Content (Join-Path $Tmp 'SHA256SUMS') | Where-Object { ($_ -split '\s+')[1] -in @($Asset, "*$Asset") } | Select-Object -First 1
        if (-not $Line) { throw "$Asset is not listed in SHA256SUMS" }
        $Expected = ($Line -split '\s+')[0].ToLower()
        $Actual = (Get-FileHash -Algorithm SHA256 (Join-Path $Tmp $Asset)).Hash.ToLower()
        if ($Expected -ne $Actual) { throw "Checksum mismatch for $Asset (expected $Expected, got $Actual)" }
        Write-Host 'gp-atlas install: checksum OK'

        $X = Join-Path $Tmp 'x'
        Expand-Archive -Path (Join-Path $Tmp $Asset) -DestinationPath $X
        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        foreach ($Bin in 'gp-atlas.exe', 'gp-atlas-beta.exe') {
            $Src = Get-ChildItem -Path $X -Recurse -Filter $Bin | Select-Object -First 1
            if (-not $Src) { throw "$Bin missing from $Asset" }
            Copy-Item -Force $Src.FullName (Join-Path $InstallDir $Bin)
        }

        $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $Parts = @($UserPath -split ';' | Where-Object { $_ })
        if ($Parts -notcontains $InstallDir) {
            [Environment]::SetEnvironmentVariable('Path', (($Parts + $InstallDir) -join ';'), 'User')
            Write-Host "gp-atlas install: added $InstallDir to your user PATH (open a new terminal)"
        }
        Write-Host "gp-atlas install: installed gp-atlas.exe and gp-atlas-beta.exe in $InstallDir"
        Write-Host 'gp-atlas install: run: gp-atlas'
        if (-not (Get-Command sf -ErrorAction SilentlyContinue)) {
            Write-Host 'gp-atlas install: note: the Salesforce CLI (sf) was not found; install it with: npm install --global @salesforce/cli@latest'
        }
    } finally {
        Remove-Item -Recurse -Force $Tmp -ErrorAction SilentlyContinue
    }
}

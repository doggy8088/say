# Install say, a cross-platform text-to-speech CLI, on Windows without Node.js.
#
#   irm https://say.gh.miniasp.com/install.ps1 | iex
#
# Options, as environment variables (set them before the line above) or as
# parameters when you run a downloaded copy of this file:
#
#   SAY_INSTALL_DIR      -InstallDir     Folder for say.exe. Default: $HOME\.local\bin
#   SAY_VERSION          -Version        Release to install, such as v0.1.0. Default: latest
#   SAY_NO_MODIFY_PATH=1 -NoModifyPath   Leave your user PATH unchanged
#
#   $env:SAY_VERSION = 'v0.1.0'; irm https://say.gh.miniasp.com/install.ps1 | iex
#
# Works in Windows PowerShell 5.1 and PowerShell 7 on x64 and ARM64.
# On macOS and Linux, use install.sh instead. Source: https://github.com/doggy8088/say
#
# All the work happens inside functions and nothing calls `exit`, so running
# this through `iex` never closes your PowerShell window.

param(
    [string]$InstallDir = $env:SAY_INSTALL_DIR,
    [string]$Version = $env:SAY_VERSION,
    [switch]$NoModifyPath
)

function Get-SayTarget {
    # An x64 PowerShell running under emulation on ARM64 reports AMD64 in its own
    # environment, so read the machine's native architecture from the registry.
    $arch = $null
    try {
        $arch = (Get-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment' -Name PROCESSOR_ARCHITECTURE).PROCESSOR_ARCHITECTURE
    } catch {
        $arch = $null
    }
    if (-not $arch) {
        $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    }
    switch ($arch) {
        'AMD64' { return 'x86_64-pc-windows-msvc' }
        'ARM64' { return 'aarch64-pc-windows-msvc' }
        default { throw "There is no prebuilt say for $arch Windows. Build it from source: cargo install --git https://github.com/doggy8088/say" }
    }
}

function Test-SayPathEntry {
    param([string]$PathValue, [string]$Dir)
    $wanted = [Environment]::ExpandEnvironmentVariables($Dir).TrimEnd('\')
    foreach ($entry in ($PathValue -split ';')) {
        if (-not $entry) { continue }
        if ([Environment]::ExpandEnvironmentVariables($entry).TrimEnd('\') -eq $wanted) { return $true }
    }
    return $false
}

function Add-SayUserPath {
    # Appends Dir to the user PATH stored under HKCU\<SubKey>. Reads the raw value and
    # writes it back as REG_EXPAND_SZ, so entries such as %USERPROFILE%\bin keep working.
    # ([Environment]::SetEnvironmentVariable would save the PATH as REG_SZ and break them.)
    param([Parameter(Mandatory = $true)][string]$Dir, [string]$SubKey = 'Environment')
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($SubKey)
    try {
        $raw = [string]$key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        if (Test-SayPathEntry -PathValue $raw -Dir $Dir) { return $false }
        $entries = @($raw -split ';' | Where-Object { $_ })
        $key.SetValue('Path', (($entries + $Dir) -join ';'), [Microsoft.Win32.RegistryValueKind]::ExpandString)
        return $true
    } finally {
        $key.Close()
    }
}

function Send-SayEnvironmentChange {
    # Setting a user variable through .NET broadcasts WM_SETTINGCHANGE, which tells
    # Explorer, and so every terminal opened from now on, to reload PATH.
    $name = 'SAY_INSTALLER_REFRESH'
    [Environment]::SetEnvironmentVariable($name, '1', 'User')
    [Environment]::SetEnvironmentVariable($name, $null, 'User')
}

# Hashing and unzipping go through .NET instead of Get-FileHash and Expand-Archive:
# Windows PowerShell 5.1 started from inside PowerShell 7 inherits PowerShell 7's
# module path and then cannot load those cmdlets.
function Get-SaySha256 {
    param([string]$Path)
    $stream = [IO.File]::OpenRead($Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        return ([BitConverter]::ToString($sha.ComputeHash($stream)) -replace '-', '').ToLowerInvariant()
    } finally {
        $sha.Dispose()
        $stream.Dispose()
    }
}

function Expand-SayZip {
    param([string]$Path, [string]$Destination)
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::ExtractToDirectory($Path, $Destination)
}

function Save-SayDownload {
    param([string]$Url, [string]$OutFile)
    try {
        Invoke-WebRequest -Uri $Url -OutFile $OutFile -UseBasicParsing
    } catch {
        throw "Could not download $Url. Check the version tag and your network connection. ($($_.Exception.Message))"
    }
}

function Install-Say {
    param([string]$InstallDir, [string]$Version, [switch]$NoModifyPath)

    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'
    # Windows PowerShell 5.1 downloads many times slower while it draws a progress bar.
    $ProgressPreference = 'SilentlyContinue'

    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        throw 'This script installs the Windows build. On macOS and Linux, run: curl -fsSL https://say.gh.miniasp.com/install.sh | sh'
    }

    # Windows PowerShell 5.1 can default to TLS 1.0, which GitHub refuses.
    try {
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    } catch {
        Write-Verbose 'Could not enable TLS 1.2; continuing with the defaults.'
    }

    $site = 'https://say.gh.miniasp.com'
    $target = Get-SayTarget
    $asset = "say-$target.zip"

    # Release files are mirrored on the website under the same layout as GitHub Releases.
    # "latest" is resolved to a tag first, so the archive and its checksum always come
    # from the same release, even while a cache still holds the previous one.
    if (-not $Version -or $Version -eq 'latest') {
        try {
            $Version = ([string](Invoke-WebRequest -Uri "$site/releases/latest.txt" -UseBasicParsing).Content).Trim()
        } catch {
            throw "Could not look up the latest release at $site/releases/latest.txt. ($($_.Exception.Message))"
        }
        if ($Version -notmatch '^v\d') {
            throw "$site/releases/latest.txt does not hold a release tag."
        }
    } elseif ($Version -notmatch '^v') {
        $Version = "v$Version"
    }
    $base = "$site/releases/download/$Version"

    if (-not $InstallDir) { $InstallDir = Join-Path $HOME '.local\bin' }
    $InstallDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallDir).TrimEnd('\')

    Write-Host "Installing say ($Version, $target) into $InstallDir"

    $work = Join-Path ([IO.Path]::GetTempPath()) ('say-install-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        $zip = Join-Path $work $asset
        Save-SayDownload -Url "$base/$asset" -OutFile $zip
        Save-SayDownload -Url "$base/$asset.sha256" -OutFile "$zip.sha256"

        # The checksum file holds "<sha256>  <file name>", possibly with CRLF line endings.
        $sumText = [string](Get-Content -LiteralPath "$zip.sha256" -Raw)
        $expected = ($sumText.Trim() -split '\s+')[0].ToLowerInvariant()
        if ($expected -notmatch '^[0-9a-f]{64}$') {
            throw "The checksum file for $asset is not in the expected format."
        }
        $actual = Get-SaySha256 -Path $zip
        if ($actual -ne $expected) {
            throw "Checksum mismatch for $asset (expected $expected, got $actual). Nothing was installed."
        }

        $unpacked = Join-Path $work 'x'
        Expand-SayZip -Path $zip -Destination $unpacked
        $exe = Join-Path $unpacked 'say.exe'
        if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
            throw "$asset does not contain say.exe."
        }

        New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
        $dest = Join-Path $InstallDir 'say.exe'
        $old = "$dest.old"
        $staged = Join-Path $InstallDir ('.say-' + [guid]::NewGuid().ToString('N') + '.tmp')
        Copy-Item -LiteralPath $exe -Destination $staged
        try {
            if (Test-Path -LiteralPath $dest) {
                # Windows will not overwrite a say.exe that is running, but it will rename it.
                Remove-Item -LiteralPath $old -Force -ErrorAction SilentlyContinue
                Move-Item -LiteralPath $dest -Destination $old -Force
            }
            Move-Item -LiteralPath $staged -Destination $dest
        } finally {
            Remove-Item -LiteralPath $staged -Force -ErrorAction SilentlyContinue
        }
        Remove-Item -LiteralPath $old -Force -ErrorAction SilentlyContinue
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }

    $installed = $null
    try {
        $ErrorActionPreference = 'Continue'
        $installed = (& $dest --version 2>&1 | Out-String).Trim()
        $exitCode = $LASTEXITCODE
    } catch {
        $installed = $_.Exception.Message
        $exitCode = -1
    } finally {
        $ErrorActionPreference = 'Stop'
    }
    if ($exitCode -ne 0) {
        throw "say.exe was installed to $dest, but it does not start: $installed"
    }
    Write-Host "Installed $installed to $dest" -ForegroundColor Green

    if (Test-SayPathEntry -PathValue $env:Path -Dir $InstallDir) {
        $found = Get-Command say -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($found -and $found.Source -ne $dest) {
            Write-Warning "Running say starts $($found.Source), which comes before $InstallDir on your PATH. You can always run this one directly: $dest"
        }
    } elseif ($NoModifyPath) {
        Write-Host ''
        Write-Host "$InstallDir is not on your PATH. Add it in Settings > System > About > Advanced system settings > Environment Variables, or run say as: $dest"
    } else {
        if (Add-SayUserPath -Dir $InstallDir) {
            Send-SayEnvironmentChange
            Write-Host "Added $InstallDir to your user PATH."
        }
        # Make say available in this window right away, not only in new ones.
        $env:Path = ($env:Path.TrimEnd(';') + ';' + $InstallDir)
    }

    Write-Host ''
    Write-Host 'Try it: say "Hello from say"'
}

$noModify = $NoModifyPath.IsPresent -or ($env:SAY_NO_MODIFY_PATH -in @('1', 'true', 'yes'))
Install-Say -InstallDir $InstallDir -Version $Version -NoModifyPath:$noModify

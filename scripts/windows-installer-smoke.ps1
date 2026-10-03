#Requires -Version 7.0
<#
Runs the shipped NSIS installer lifecycle on a disposable GitHub-hosted runner.
This does not launch RambleDesk or validate its UI, SQLite migrations, or updater UI.
There is deliberately no local-run override: /D and /NS do not isolate registry writes.
#>
[CmdletBinding()]
param(
    [string]$InstallerPath,
    [string]$OutputDirectory,
    [ValidateRange(30, 600)][int]$ProcessTimeoutSeconds = 300
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# This guard must precede downloads, filesystem writes, registry access, and processes.
if (-not $IsWindows -or $env:GITHUB_ACTIONS -cne 'true' -or
    $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or
    $env:RUNNER_OS -cne 'Windows' -or $env:RUNNER_ARCH -cne 'X64' -or
    $env:GITHUB_RUN_ID -notmatch '^\d+$' -or $env:GITHUB_RUN_ATTEMPT -notmatch '^\d+$') {
    throw 'Installer smoke test requires a disposable GitHub-hosted Windows X64 runner; local and self-hosted execution is forbidden.'
}

. (Join-Path $PSScriptRoot 'windows-installer-binary.ps1')

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $env:GITHUB_WORKSPACE -or
    [IO.Path]::GetFullPath($env:GITHUB_WORKSPACE) -ne $repoRoot -or
    -not $env:RUNNER_TEMP -or -not (Test-Path -LiteralPath $env:RUNNER_TEMP -PathType Container)) {
    throw 'Installer smoke test requires the checked-out GitHub workspace and runner temporary directory.'
}
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $env:RUNNER_TEMP 'windows-installer-smoke' }
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$tempPrefix = [IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\') + '\'
if (-not $OutputDirectory.StartsWith($tempPrefix, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Smoke output must be a child directory of RUNNER_TEMP.'
}
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Smoke output directory must not already exist.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$logPath = Join-Path $OutputDirectory 'smoke.log'
$reportPath = Join-Path $OutputDirectory 'report.json'
$report = [ordered]@{
    schema_version = 1
    status = 'running'
    scope = 'Real NSIS install, overwrite upgrade, and uninstall with synthetic file preservation; no application launch, UI, or database migration validation.'
    started_at = [DateTime]::UtcNow.ToString('o')
    run_id = $env:GITHUB_RUN_ID
    run_attempt = $env:GITHUB_RUN_ATTEMPT
    commit = $env:GITHUB_SHA
    checks = [Collections.Generic.List[object]]::new()
    processes = [Collections.Generic.List[object]]::new()
}

function Write-SmokeLog([string]$Message) {
    $line = '[{0}] {1}' -f [DateTime]::UtcNow.ToString('o'), $Message
    Add-Content -LiteralPath $logPath -Value $line -Encoding utf8
    Write-Host $line
}

function Assert-Smoke([bool]$Condition, [string]$Message) {
    $report.checks.Add([ordered]@{ check = $Message; passed = $Condition })
    Write-SmokeLog "$(if ($Condition) { 'PASS' } else { 'FAIL' }) $Message"
    if (-not $Condition) { throw $Message }
}

function Invoke-Installer([string]$Name, [string]$Path, [string]$Arguments) {
    Write-SmokeLog "$Name start: $Path $Arguments"
    $record = [ordered]@{ name = $Name; path = $Path; arguments = $Arguments; started_at = [DateTime]::UtcNow.ToString('o'); exit_code = $null; timed_out = $false }
    $report.processes.Add($record)
    $process = Start-Process -FilePath $Path -ArgumentList $Arguments -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput (Join-Path $OutputDirectory "$Name.stdout.log") `
        -RedirectStandardError (Join-Path $OutputDirectory "$Name.stderr.log")
    try {
        if (-not $process.WaitForExit($ProcessTimeoutSeconds * 1000)) {
            $record.timed_out = $true
            # Only terminate this process and its descendants on the disposable runner.
            $process.Kill($true)
            throw "$Name exceeded $ProcessTimeoutSeconds seconds."
        }
        $process.WaitForExit()
        $record.exit_code = $process.ExitCode
        Assert-Smoke ($process.ExitCode -eq 0) "$Name exits with code 0 (actual $($process.ExitCode))"
    } finally {
        $record.finished_at = [DateTime]::UtcNow.ToString('o')
        $process.Dispose()
    }
}

function Read-Installation([string]$ExpectedVersion, [string]$Phase) {
    Assert-Smoke (Test-Path -LiteralPath $uninstallKey) "$Phase has uninstall registration"
    $registration = Get-ItemProperty -LiteralPath $uninstallKey
    Assert-Smoke ($registration.DisplayVersion -eq $ExpectedVersion) "$Phase registry version is $ExpectedVersion"
    Assert-Smoke ($registration.DisplayName -eq $config.productName) "$Phase product name matches"
    Assert-Smoke ($registration.Publisher -eq $config.bundle.publisher) "$Phase publisher matches"
    $directory = [IO.Path]::GetFullPath($registration.InstallLocation.Trim('"'))
    Assert-Smoke ($directory -eq $installDirectory) "$Phase uses the default current-user installation path"
    Assert-Smoke (Test-Path -LiteralPath $productKey) "$Phase saves its product install location"
    Assert-Smoke ((Get-Item -LiteralPath $productKey).GetValue('') -eq $directory) "$Phase saved product location matches"
    Assert-Smoke ($registration.MainBinaryName -match '^[^\\/:]+\.exe$') "$Phase main binary is a filename"
    $binary = Join-Path $directory $registration.MainBinaryName
    Assert-Smoke (Test-Path -LiteralPath $binary -PathType Leaf) "$Phase installed executable exists"
    $versionInfo = (Get-Item -LiteralPath $binary).VersionInfo
    Assert-Smoke ($versionInfo.ProductVersion -eq $ExpectedVersion) "$Phase executable product version is $ExpectedVersion"
    Assert-Smoke ((Get-Item -LiteralPath $binary).Length -gt 0) "$Phase executable is nonempty"
    $uninstaller = Join-Path $directory 'uninstall.exe'
    Assert-Smoke (Test-Path -LiteralPath $uninstaller -PathType Leaf) "$Phase installed uninstaller exists"
    Assert-Smoke ($registration.UninstallString.Trim('"') -eq $uninstaller) "$Phase uninstall command points to the installed uninstaller"
    foreach ($shortcut in $shortcuts) {
        Assert-Smoke (Test-Path -LiteralPath $shortcut -PathType Leaf) "$Phase shortcut exists: $shortcut"
    }
    return [ordered]@{
        version = $ExpectedVersion; directory = $directory; binary = $binary
        binary_sha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
        uninstaller = $uninstaller
    }
}

function Assert-Markers([string]$Phase) {
    foreach ($marker in $report.markers) {
        Assert-Smoke (Test-Path -LiteralPath $marker.path -PathType Leaf) "$Phase preserves marker: $($marker.path)"
        Assert-Smoke ((Get-FileHash -LiteralPath $marker.path -Algorithm SHA256).Hash -eq $marker.sha256) "$Phase preserves marker contents: $($marker.path)"
    }
}

try {
    Write-SmokeLog 'Hosted Windows runner guard passed. Application launch and native draft migration are outside this test.'
    Assert-Smoke ([bool]$InstallerPath) 'Current NSIS installer path was supplied'
    $InstallerPath = (Resolve-Path -LiteralPath $InstallerPath).Path
    Assert-Smoke ($InstallerPath.StartsWith($repoRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
        $InstallerPath -match '\\bundle\\nsis\\[^\\]+_x64-setup\.exe$') 'Current installer is the x64 NSIS artifact inside the checkout'
    $configDirectory = Join-Path $repoRoot 'apps/desktop/src-tauri'
    $config = Get-Content -LiteralPath (Join-Path $configDirectory 'tauri.conf.json') -Raw | ConvertFrom-Json
    Assert-Smoke ($config.bundle.windows.nsis.installMode -eq 'currentUser') 'Installer uses currentUser mode'
    $installDirectory = Join-Path $env:LOCALAPPDATA $config.productName
    $uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$($config.productName)"
    $productKey = "HKCU:\Software\$($config.bundle.publisher)\$($config.productName)"
    $appDataDirectories = @(
        $installDirectory,
        (Join-Path $env:APPDATA $config.identifier),
        (Join-Path $env:LOCALAPPDATA $config.identifier)
    )
    $shortcuts = @(
        (Join-Path ([Environment]::GetFolderPath('Programs')) "$($config.productName).lnk"),
        (Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) "$($config.productName).lnk")
    )
    foreach ($path in @($uninstallKey, $productKey,
        "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$($config.productName)",
        "HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\$($config.productName)") + $appDataDirectories + $shortcuts) {
        Assert-Smoke (-not (Test-Path -LiteralPath $path)) "Fresh runner has no pre-existing product state: $path"
    }
    Assert-Smoke (@(Get-Process -Name 'rambledesk' -ErrorAction SilentlyContinue).Count -eq 0) 'No RambleDesk process is running'
    $report.current = [ordered]@{
        version = $config.version; installer = $InstallerPath
        installer_sha256 = (Get-FileHash -LiteralPath $InstallerPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }

    Assert-Smoke ($env:GITHUB_REPOSITORY -match '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$') 'GitHub repository name is valid'
    $headers = @{ Accept = 'application/vnd.github+json'; 'X-GitHub-Api-Version' = '2022-11-28' }
    if ($env:GH_TOKEN) { $headers.Authorization = "Bearer $env:GH_TOKEN" }
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$env:GITHUB_REPOSITORY/releases/latest" -Headers $headers -TimeoutSec 60
    Assert-Smoke (-not $release.draft -and -not $release.prerelease -and $release.tag_name -match '^v\d+\.\d+\.\d+$') 'Latest baseline release is a published stable SemVer tag'
    $baselineVersion = $release.tag_name.Substring(1)
    Assert-Smoke ([System.Management.Automation.SemanticVersion]$config.version -gt [System.Management.Automation.SemanticVersion]$baselineVersion) 'Current version is newer than the stable baseline'
    $baselineAssets = @($release.assets | Where-Object { $_.name -eq "$($config.productName)_${baselineVersion}_x64-setup.exe" })
    $checksumAssets = @($release.assets | Where-Object { $_.name -eq 'SHA256SUMS.txt' })
    Assert-Smoke ($baselineAssets.Count -eq 1 -and $checksumAssets.Count -eq 1) 'Baseline contains one x64 NSIS installer and SHA256SUMS.txt'
    $downloadDirectory = Join-Path $OutputDirectory 'downloads'
    New-Item -ItemType Directory -Path $downloadDirectory | Out-Null
    foreach ($asset in @($baselineAssets[0], $checksumAssets[0])) {
        $uri = [Uri]$asset.browser_download_url
        Assert-Smoke ($uri.Scheme -eq 'https' -and $uri.Host -eq 'github.com' -and
            $uri.AbsolutePath.StartsWith("/$env:GITHUB_REPOSITORY/releases/download/", [StringComparison]::Ordinal)) "Baseline asset is from this repository: $($asset.name)"
        Invoke-WebRequest -Uri $uri -OutFile (Join-Path $downloadDirectory $asset.name) -TimeoutSec 180
    }
    $baselineInstaller = Join-Path $downloadDirectory $baselineAssets[0].name
    $checksumPattern = '^([0-9a-fA-F]{64})\s+\*?' + [Regex]::Escape($baselineAssets[0].name) + '$'
    $checksumLines = @(Get-Content -LiteralPath (Join-Path $downloadDirectory 'SHA256SUMS.txt') | Where-Object { $_ -match $checksumPattern })
    Assert-Smoke ($checksumLines.Count -eq 1) 'Baseline installer has exactly one SHA-256 entry'
    $expectedHash = [Regex]::Match($checksumLines[0], $checksumPattern).Groups[1].Value
    $baselineHash = (Get-FileHash -LiteralPath $baselineInstaller -Algorithm SHA256).Hash
    Assert-Smoke ($baselineHash -eq $expectedHash) 'Downloaded baseline installer matches published SHA-256'
    $report.baseline = [ordered]@{ tag = $release.tag_name; release_id = $release.id; asset = $baselineAssets[0].name; installer_sha256 = $baselineHash.ToLowerInvariant() }
    Invoke-Installer 'baseline-install' $baselineInstaller '/S'
    $report.baseline.installation = Read-Installation $baselineVersion 'Baseline'
    Assert-Smoke (Test-Path -LiteralPath (Join-Path $installDirectory 'THIRD_PARTY_NOTICES.md') -PathType Leaf) 'Baseline bundles third-party notices'

    # Real storage defaults are LOCALAPPDATA/RambleDesk/{state,library}. These are
    # explicitly synthetic files, not a fabricated database or native saved draft.
    $markerDirectories = @((Join-Path $installDirectory 'state'), (Join-Path $installDirectory 'library')) + $appDataDirectories[1..2]
    $report.markers = @($markerDirectories | ForEach-Object {
        New-Item -ItemType Directory -Path $_ -Force | Out-Null
        $markerPath = Join-Path $_ "installer-smoke-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT.txt"
        [IO.File]::WriteAllText($markerPath, "Synthetic installer preservation marker: $([Guid]::NewGuid())")
        [ordered]@{ path = $markerPath; sha256 = (Get-FileHash -LiteralPath $markerPath -Algorithm SHA256).Hash }
    })
    Write-SmokeLog "Created $($report.markers.Count) synthetic preservation markers."
    Invoke-Installer 'current-upgrade' $InstallerPath '/S /UPDATE'
    $report.current.installation = Read-Installation $config.version 'Upgraded'
    Assert-Smoke ($report.current.installation.directory -eq $report.baseline.installation.directory) 'Upgrade preserves the baseline install location'
    Assert-Smoke ($report.current.installation.binary_sha256 -ne $report.baseline.installation.binary_sha256) 'Upgrade replaces the baseline executable'
    $buildDirectory = Split-Path (Split-Path (Split-Path $InstallerPath -Parent) -Parent) -Parent
    $builtBinary = Join-Path $buildDirectory (Split-Path $report.current.installation.binary -Leaf)
    $report.current.binary_verification = Compare-TauriNsisBinary `
        -BuildBytes ([IO.File]::ReadAllBytes($builtBinary)) `
        -InstalledBytes ([IO.File]::ReadAllBytes($report.current.installation.binary))
    $report.current.binary_verification.build_path = $builtBinary
    Assert-Smoke $report.current.binary_verification.passed `
        "Installed executable matches the complete NSIS payload from this build: $($report.current.binary_verification.errors -join '; ')"
    Assert-Markers 'Upgrade'

    $report.resources = @($config.bundle.resources.PSObject.Properties | ForEach-Object {
        $source = [IO.Path]::GetFullPath((Join-Path $configDirectory $_.Name))
        $installed = [IO.Path]::GetFullPath((Join-Path $installDirectory ([string]$_.Value)))
        Assert-Smoke ($source.StartsWith($repoRoot + '\', [StringComparison]::OrdinalIgnoreCase) -and
            $installed.StartsWith($installDirectory + '\', [StringComparison]::OrdinalIgnoreCase)) "Resource paths stay inside checkout and installation: $($_.Value)"
        Assert-Smoke (Test-Path -LiteralPath $installed -PathType Leaf) "Upgraded resource exists: $($_.Value)"
        $expected = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
        $actual = (Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash
        Assert-Smoke ($actual -eq $expected) "Upgraded resource matches the build source: $($_.Value)"
        [ordered]@{ target = $_.Value; installed = $installed; sha256 = $actual.ToLowerInvariant() }
    })

    # NSIS _?= suppresses the uninstaller's temporary child process, so timeout
    # and exit status cover the actual uninstall. Run an identical copy outside
    # INSTDIR so the installed uninstall.exe can also be removed. _?= must be last
    # and unquoted, including paths with spaces (NSIS manual, section 3.2.2).
    $uninstallerCopy = Join-Path $downloadDirectory 'uninstall-current.exe'
    Copy-Item -LiteralPath $report.current.installation.uninstaller -Destination $uninstallerCopy
    Invoke-Installer 'current-uninstall' $uninstallerCopy "/S _?=$installDirectory"
    Assert-Smoke (-not (Test-Path -LiteralPath $uninstallKey)) 'Uninstall removes Add/Remove Programs registration'
    foreach ($path in @($report.current.installation.binary, $report.current.installation.uninstaller) + $shortcuts + @($report.resources.installed)) {
        Assert-Smoke (-not (Test-Path -LiteralPath $path)) "Uninstall removes installed file: $path"
    }
    Assert-Markers 'Uninstall'
    $report.status = 'passed'
    Write-SmokeLog 'Installer lifecycle passed; native UI and database migration remain separate acceptance checks.'
} catch {
    $report.status = 'failed'
    $report.error = $_.Exception.Message
    Write-SmokeLog "ERROR: $($_.Exception.Message)"
    throw
} finally {
    $report.finished_at = [DateTime]::UtcNow.ToString('o')
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $reportPath -Encoding utf8
    Write-SmokeLog "Report: $reportPath"
}

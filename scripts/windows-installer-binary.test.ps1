#Requires -Version 7.0
# Pure regression tests. No installer or application binary is executed.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'windows-installer-binary.ps1')

$passed = 0
function Assert-Test([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Test-Case([string]$Name, [scriptblock]$Body) {
    & $Body
    $script:passed += 1
    Write-Host "PASS $Name"
}
function New-TestBinary([string]$Code = 'UNK', [int]$MarkerOffset = 512) {
    $bytes = [byte[]]::new(1024)
    $bytes[0] = 0x4d; $bytes[1] = 0x5a
    [Array]::Copy([BitConverter]::GetBytes([uint32]0x80), 0, $bytes, 0x3c, 4)
    [Array]::Copy([BitConverter]::GetBytes([uint32]0x4550), 0, $bytes, 0x80, 4)
    [Array]::Copy([BitConverter]::GetBytes([uint16]0x8664), 0, $bytes, 0x84, 2)
    [Array]::Copy([BitConverter]::GetBytes([uint16]0xf0), 0, $bytes, 0x94, 2)
    [Array]::Copy([BitConverter]::GetBytes([uint16]0x20b), 0, $bytes, 0x98, 2)
    [Array]::Copy([BitConverter]::GetBytes([uint32]16), 0, $bytes, 0x98 + 108, 4)
    $bytes[450] = 0xff # Non-ASCII bytes must not shift marker offsets.
    Set-TestMarker $bytes $Code $MarkerOffset
    return ,$bytes
}
function Set-TestMarker([byte[]]$Bytes, [string]$Code, [int]$Offset = 512) {
    $marker = [Text.Encoding]::ASCII.GetBytes('__TAURI_BUNDLE_TYPE_VAR_' + $Code)
    [Array]::Copy($marker, 0, $Bytes, $Offset, $marker.Length)
}
function Assert-Rejected([byte[]]$Build, [byte[]]$Installed, [string]$Reason) {
    $beforeBuild = [Convert]::ToBase64String($Build)
    $beforeInstalled = [Convert]::ToBase64String($Installed)
    $result = Compare-TauriNsisBinary -BuildBytes $Build -InstalledBytes $Installed
    Assert-Test (-not $result.passed) 'Corrupt or unsupported input was accepted'
    Assert-Test ([bool]($result.errors | Where-Object { $_ -like "*$Reason*" })) "Missing rejection reason: $Reason"
    Assert-Test ([Convert]::ToBase64String($Build) -ceq $beforeBuild) 'Rejected build input changed'
    Assert-Test ([Convert]::ToBase64String($Installed) -ceq $beforeInstalled) 'Rejected installed input changed'
}

Test-Case 'only the expected marker transition passes and neither input is changed' {
    $build = New-TestBinary 'UNK'; $installed = New-TestBinary 'NSS'
    $beforeBuild = [Convert]::ToBase64String($build)
    $beforeInstalled = [Convert]::ToBase64String($installed)
    $result = Compare-TauriNsisBinary $build $installed
    Assert-Test $result.passed ($result.errors -join '; ')
    Assert-Test ($result.build_marker.offset -eq 512 -and $result.installed_marker.offset -eq 512) 'Wrong byte offset'
    Assert-Test ($result.raw_build_sha256 -ne $result.expected_nsis_sha256) 'Raw and packaged hashes must differ'
    Assert-Test ($result.expected_nsis_sha256 -eq $result.installed_sha256) 'Expected and installed hashes differ'
    Assert-Test ($result.installed_sha256 -eq (Get-InstallerByteSha256 $installed)) 'Installed bytes were normalized'
    Assert-Test ([Convert]::ToBase64String($build) -ceq $beforeBuild) 'Build input changed'
    Assert-Test ([Convert]::ToBase64String($installed) -ceq $beforeInstalled) 'Installed input changed'
}
Test-Case 'a changed byte outside the marker fails full-file integrity' {
    $installed = New-TestBinary 'NSS'; $installed[900] = 1
    Assert-Rejected (New-TestBinary) $installed 'complete expected NSIS payload'
}
Test-Case 'a changed PE checksum is not normalized away' {
    $installed = New-TestBinary 'NSS'; $installed[0x98 + 64] = 1
    Assert-Rejected (New-TestBinary) $installed 'complete expected NSIS payload'
}
Test-Case 'appended data fails length and full-file integrity' {
    $installed = [byte[]]((New-TestBinary 'NSS') + [byte]1)
    Assert-Rejected (New-TestBinary) $installed 'byte lengths differ'
    Assert-Rejected (New-TestBinary) $installed 'complete expected NSIS payload'
}
Test-Case 'missing build marker is rejected' {
    $build = New-TestBinary; [Array]::Clear($build, 512, 27)
    Assert-Rejected $build (New-TestBinary 'NSS') 'Build must contain exactly one'
}
Test-Case 'duplicate build UNK marker is rejected' {
    $build = New-TestBinary; Set-TestMarker $build 'UNK' 700
    Assert-Rejected $build (New-TestBinary 'NSS') 'Build must contain exactly one'
}
foreach ($code in @('NSS', 'MSI', 'BAD')) {
    Test-Case "prepatched or wrong build marker $code is rejected" {
        Assert-Rejected (New-TestBinary $code) (New-TestBinary 'NSS') 'Build must contain exactly one'
    }
    Test-Case "build UNK plus an extra $code marker is rejected" {
        $build = New-TestBinary; Set-TestMarker $build $code 700
        Assert-Rejected $build (New-TestBinary 'NSS') 'Build must contain exactly one'
    }
}
Test-Case 'missing installed marker is rejected' {
    $installed = New-TestBinary 'NSS'; [Array]::Clear($installed, 512, 27)
    Assert-Rejected (New-TestBinary) $installed 'Installed file must contain exactly one'
}
Test-Case 'duplicate installed NSS marker is rejected' {
    $installed = New-TestBinary 'NSS'; Set-TestMarker $installed 'NSS' 700
    Assert-Rejected (New-TestBinary) $installed 'Installed file must contain exactly one'
}
foreach ($code in @('UNK', 'MSI', 'BAD')) {
    Test-Case "wrong installed marker $code is rejected" {
        Assert-Rejected (New-TestBinary) (New-TestBinary $code) 'Installed file must contain exactly one'
    }
    Test-Case "installed NSS plus an extra $code marker is rejected" {
        $installed = New-TestBinary 'NSS'; Set-TestMarker $installed $code 700
        Assert-Rejected (New-TestBinary) $installed 'Installed file must contain exactly one'
    }
}
Test-Case 'a valid NSS marker at another offset is rejected' {
    Assert-Rejected (New-TestBinary) (New-TestBinary 'NSS' 600) 'marker offsets differ'
}
Test-Case 'a truncated marker is rejected' {
    $installed = New-TestBinary 'NSS'; [Array]::Clear($installed, 512, 27)
    $prefix = [Text.Encoding]::ASCII.GetBytes('__TAURI_BUNDLE_TYPE_VAR_')
    [Array]::Copy($prefix, 0, $installed, $installed.Length - $prefix.Length, $prefix.Length)
    Assert-Rejected (New-TestBinary) $installed 'Installed file must contain exactly one'
}
Test-Case 'empty and truncated PE data fail closed' {
    Assert-Rejected ([byte[]]::new(0)) ([byte[]]::new(0)) 'Missing DOS executable header'
    $installed = New-TestBinary 'NSS'
    [Array]::Copy([BitConverter]::GetBytes([uint32]::MaxValue), 0, $installed, 0x3c, 4)
    Assert-Rejected (New-TestBinary) $installed 'truncated PE header'
}
Test-Case 'unsupported PE machine and optional-header layouts fail closed' {
    $installed = New-TestBinary 'NSS'; $installed[0x84] = 0
    Assert-Rejected (New-TestBinary) $installed 'Expected a complete Windows x64 PE32+'
    $installed = New-TestBinary 'NSS'; $installed[0x94] = 1; $installed[0x95] = 0
    Assert-Rejected (New-TestBinary) $installed 'Expected a complete Windows x64 PE32+'
}
Test-Case 'Authenticode certificate pointers or sizes fail even when both payloads agree' {
    foreach ($field in @(144, 148)) {
        $build = New-TestBinary; $installed = New-TestBinary 'NSS'
        $build[0x98 + $field] = 1; $installed[0x98 + $field] = 1
        Assert-Rejected $build $installed 'Authenticode certificate data'
    }
}
Test-Case 'PowerShell source parses without errors' {
    foreach ($file in @('windows-installer-binary.ps1', 'windows-installer-smoke.ps1')) {
        $tokens = $null; $parseErrors = $null
        [void][Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot $file), [ref]$tokens, [ref]$parseErrors)
        Assert-Test ($parseErrors.Count -eq 0) "Parse error in $file"
    }
}
Test-Case 'local invocation stops at the hosted-only guard before helper loading or side effects' {
    $previous = $env:GITHUB_ACTIONS
    try {
        # Only disable authorization for this negative test; never impersonate a hosted runner.
        $env:GITHUB_ACTIONS = 'false'
        $guardError = $null
        try { & (Join-Path $PSScriptRoot 'windows-installer-smoke.ps1') -InstallerPath 'must-not-be-opened.exe' }
        catch { $guardError = $_.Exception.Message }
        Assert-Test ($guardError -eq 'Installer smoke test requires a disposable GitHub-hosted Windows X64 runner; local and self-hosted execution is forbidden.') 'Hosted guard was bypassed or changed'
    } finally { $env:GITHUB_ACTIONS = $previous }
}
Write-Host "Windows installer binary tests: $passed passed, 0 failed. No installer was executed."

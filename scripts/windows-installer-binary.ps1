#Requires -Version 7.0
# Pure byte comparison; loading this file never reads/writes files or starts a process.
# Tauri CLI 2.11.4 patches UNK to NSS for NSIS, then restores the raw build executable:
# https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle.rs

function Get-InstallerByteSha256([byte[]]$Bytes) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($sha.ComputeHash($Bytes)).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function Get-TauriBundleMarker([byte[]]$Bytes, [string]$ExpectedCode) {
    $prefix = '__TAURI_BUNDLE_TYPE_VAR_'
    # Latin-1 preserves one character per byte, including arbitrary non-ASCII PE data.
    $text = [Text.Encoding]::GetEncoding(28591).GetString($Bytes)
    $offsets = [Collections.Generic.List[int]]::new()
    $offset = $text.IndexOf($prefix, [StringComparison]::Ordinal)
    while ($offset -ge 0) {
        $offsets.Add($offset)
        $offset = $text.IndexOf($prefix, $offset + 1, [StringComparison]::Ordinal)
    }
    $marker = [ordered]@{ count = $offsets.Count; offset = $null; value = $null; valid = $false }
    if ($offsets.Count -eq 1) {
        $marker.offset = $offsets[0]
        $length = $prefix.Length + 3
        if ($marker.offset + $length -le $text.Length) {
            $marker.value = $text.Substring($marker.offset, $length)
            $marker.valid = $marker.value -ceq ($prefix + $ExpectedCode)
        }
    }
    return $marker
}

function Get-UnsignedInstallerPe([byte[]]$Bytes) {
    # This gate only supports the current unsigned Windows x64 payload. In particular,
    # Authenticode changes are not normalized away: any certificate directory fails closed.
    $result = [ordered]@{ valid = $false; certificate_offset = $null; certificate_size = $null; error = $null }
    if ($Bytes.Length -lt 64 -or $Bytes[0] -ne 0x4d -or $Bytes[1] -ne 0x5a) {
        $result.error = 'Missing DOS executable header'; return $result
    }
    $peOffset = [long][BitConverter]::ToUInt32($Bytes, 0x3c)
    if ($peOffset -lt 64 -or $peOffset + 24 -gt $Bytes.Length -or
        [BitConverter]::ToUInt32($Bytes, [int]$peOffset) -ne 0x00004550) {
        $result.error = 'Missing or truncated PE header'; return $result
    }
    $optionalOffset = [int]$peOffset + 24
    $optionalSize = [BitConverter]::ToUInt16($Bytes, [int]$peOffset + 20)
    if ([BitConverter]::ToUInt16($Bytes, [int]$peOffset + 4) -ne 0x8664 -or
        $optionalSize -lt 152 -or [long]$optionalOffset + $optionalSize -gt $Bytes.Length -or
        [BitConverter]::ToUInt16($Bytes, $optionalOffset) -ne 0x20b -or
        [BitConverter]::ToUInt32($Bytes, $optionalOffset + 108) -lt 5) {
        $result.error = 'Expected a complete Windows x64 PE32+ certificate directory'; return $result
    }
    $result.certificate_offset = [BitConverter]::ToUInt32($Bytes, $optionalOffset + 144)
    $result.certificate_size = [BitConverter]::ToUInt32($Bytes, $optionalOffset + 148)
    if ($result.certificate_offset -ne 0 -or $result.certificate_size -ne 0) {
        $result.error = 'Authenticode certificate data requires a separate signature-aware verification'; return $result
    }
    $result.valid = $true
    return $result
}

function Compare-TauriNsisBinary {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][byte[]]$BuildBytes,
        [Parameter(Mandatory)][AllowEmptyCollection()][byte[]]$InstalledBytes
    )
    $errors = [Collections.Generic.List[string]]::new()
    $buildMarker = Get-TauriBundleMarker $BuildBytes 'UNK'
    $installedMarker = Get-TauriBundleMarker $InstalledBytes 'NSS'
    $buildPe = Get-UnsignedInstallerPe $BuildBytes
    $installedPe = Get-UnsignedInstallerPe $InstalledBytes
    $result = [ordered]@{
        passed = $false
        comparison = 'Tauri CLI 2.11.4: unique UNK to NSS, full-file SHA-256'
        raw_build_sha256 = Get-InstallerByteSha256 $BuildBytes
        expected_nsis_sha256 = $null
        installed_sha256 = Get-InstallerByteSha256 $InstalledBytes
        build_length = $BuildBytes.Length
        installed_length = $InstalledBytes.Length
        build_marker = $buildMarker
        installed_marker = $installedMarker
        build_pe = $buildPe
        installed_pe = $installedPe
        errors = $errors
    }
    if (-not $buildMarker.valid) { $errors.Add('Build must contain exactly one bundle marker, with value UNK') }
    if (-not $installedMarker.valid) { $errors.Add('Installed file must contain exactly one bundle marker, with value NSS') }
    if (-not $buildPe.valid) { $errors.Add("Build PE: $($buildPe.error)") }
    if (-not $installedPe.valid) { $errors.Add("Installed PE: $($installedPe.error)") }
    if ($BuildBytes.Length -ne $InstalledBytes.Length) { $errors.Add('Executable byte lengths differ') }
    if ($buildMarker.valid -and $installedMarker.valid -and $buildMarker.offset -ne $installedMarker.offset) {
        $errors.Add('Bundle marker offsets differ')
    }
    if ($buildMarker.valid -and $buildPe.valid) {
        # Only a private expected copy is patched. Never normalize the installed bytes.
        $expected = [byte[]]$BuildBytes.Clone()
        $replacement = [Text.Encoding]::ASCII.GetBytes('__TAURI_BUNDLE_TYPE_VAR_NSS')
        [Array]::Copy($replacement, 0, $expected, $buildMarker.offset, $replacement.Length)
        $result.expected_nsis_sha256 = Get-InstallerByteSha256 $expected
        if ($result.expected_nsis_sha256 -cne $result.installed_sha256) {
            $errors.Add('Installed SHA-256 differs from the complete expected NSIS payload')
        }
    }
    $result.passed = $errors.Count -eq 0
    return $result
}

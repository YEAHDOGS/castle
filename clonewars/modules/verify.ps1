# ==============================================================================
# Castle VM -- Cryptographic Integrity Verification Engine
# ==============================================================================
# Three layers of verification:
#   1. Test-CryptographicHash  -- SHA256/SHA512/SHA1/MD5 hash comparison
#   2. Test-GpgSignature       -- Detached GPG signature verification
#   3. Test-IsoIntegrity       -- High-level wrapper combining both
#
# Supports:
#   - Remote hash manifests (Linux distro checksum files)
#   - Static hardcoded hashes (Windows / macOS)
#   - Archive.org XML _files.xml manifests
#   - Standard Linux format: "hash  filename"
#   - BSD/RHEL format: "ALGO(filename) = hash"
# ==============================================================================

function Get-FileHashDotNet {
    param([string]$Path, [string]$Algorithm = "SHA512")
    $stream = [System.IO.File]::OpenRead($Path)
    if ($Algorithm -eq "SHA512") { $hasher = [System.Security.Cryptography.SHA512]::Create() }
    elseif ($Algorithm -eq "SHA256") { $hasher = [System.Security.Cryptography.SHA256]::Create() }
    elseif ($Algorithm -eq "MD5") { $hasher = [System.Security.Cryptography.MD5]::Create() }
    elseif ($Algorithm -eq "SHA1") { $hasher = [System.Security.Cryptography.SHA1]::Create() }
    else { $hasher = [System.Security.Cryptography.SHA256]::Create() }
    $hashBytes = $hasher.ComputeHash($stream)
    $stream.Close()
    $hasher.Dispose()
    return ([System.BitConverter]::ToString($hashBytes) -replace '-').ToLower()
}

function Test-CryptographicHash {
    param (
        [string]$FilePath,
        [string]$ExpectedHash,
        [string]$HashUrl,
        [string]$Algorithm = "SHA512",
        [string]$IsoName,
        [object]$HttpClient
    )

    Write-Host "  [HASH] Verifying $Algorithm integrity..." -ForegroundColor Yellow

    # -- Fetch remote hash if a URL was provided --
    if (-not [string]::IsNullOrEmpty($HashUrl)) {
        try {
            Write-Host "     [>] Fetching remote manifest from: $HashUrl" -ForegroundColor DarkGray
            $FetchedData = $HttpClient.GetStringAsync($HashUrl).GetAwaiter().GetResult()
            Write-Host "     [+] Retrieved content from link (first 200 chars): $(if ($FetchedData) { $FetchedData.Substring(0, [Math]::Min(200, $FetchedData.Length)) -replace '\r?\n', ' ' } else { 'empty' })" -ForegroundColor DarkGray

            # -- Attempt 1: XML manifest (Archive.org _files.xml format) --
            if ($FetchedData.TrimStart().StartsWith("<")) {
                try {
                    [xml]$XmlDoc = $FetchedData
                    $AlgoLower = $Algorithm.ToLower()

                    if ($XmlDoc.files -and $XmlDoc.files.file) {
                        $TargetName = if (-not [string]::IsNullOrEmpty($IsoName)) {
                            $IsoName
                        }
                        else {
                            [System.IO.Path]::GetFileName($FilePath)
                        }

                        $FileNode = $XmlDoc.files.file | Where-Object { $_.name -eq $TargetName }

                        if ($FileNode -and $FileNode.$AlgoLower) {
                            $ExpectedHash = [string]$FileNode.$AlgoLower
                            Write-Host "     [+] Found $Algorithm hash in XML manifest for: $TargetName" -ForegroundColor DarkGray
                        }
                    }
                }
                catch {
                    Write-Host "     [?] XML parsing failed, falling back to text matching..." -ForegroundColor DarkGray
                }
            }

            # -- Attempt 2: Text manifest (only if XML did not yield a hash) --
            if ([string]::IsNullOrEmpty($ExpectedHash)) {
                if (-not [string]::IsNullOrEmpty($IsoName)) {
                    # Multi-line manifest: find the line containing our specific ISO filename
                    $EscapedIsoName = [regex]::Escape($IsoName)

                    # Pattern 1 (Standard Linux): hash  *filename
                    # Pattern 2 (BSD / RHEL):     ALGO(filename) = hash
                    $Pattern = "(?mi)(?:^([a-fA-F0-9]+)\s+\*?${EscapedIsoName}\s*$|^\w+\s*\(${EscapedIsoName}\)\s*=\s*([a-fA-F0-9]+)\s*$)"

                    if ($FetchedData -match $Pattern) {
                        $ExpectedHash = if (![string]::IsNullOrEmpty($Matches[1])) { $Matches[1] } else { $Matches[2] }
                    }
                    else {
                        # Proximity fallback: Find hex string of matching length within 250 chars of the filename
                        $Length = switch ($Algorithm) {
                            "MD5" { 32 }
                            "SHA1" { 40 }
                            "SHA256" { 64 }
                            "SHA512" { 128 }
                            default { 64 }
                        }
                        $FallbackPattern1 = "(?is)${EscapedIsoName}.{1,250}?([a-fA-F0-9]{$Length})"
                        $FallbackPattern2 = "(?is)([a-fA-F0-9]{$Length}).{1,250}?${EscapedIsoName}"

                        if ($FetchedData -match $FallbackPattern1) {
                            $ExpectedHash = $Matches[1]
                            Write-Host "     [+] Found $Algorithm hash via proximity fallback near filename: $ExpectedHash" -ForegroundColor DarkGray
                        }
                        elseif ($FetchedData -match $FallbackPattern2) {
                            $ExpectedHash = $Matches[1]
                            Write-Host "     [+] Found $Algorithm hash via proximity fallback near filename: $ExpectedHash" -ForegroundColor DarkGray
                        }
                        elseif ($FetchedData.Trim() -match "^([a-fA-F0-9]{$Length})$") {
                            # Bare checksum file: just the hash, no filename column
                            # (e.g. Knulli's per-image .sha256/.md5 release assets)
                            $ExpectedHash = $Matches[1]
                            Write-Host "     [+] Manifest is a bare $Algorithm checksum file: $ExpectedHash" -ForegroundColor DarkGray
                        }
                        else {
                            Write-Host "     [?] File '$IsoName' not found in remote manifest." -ForegroundColor Yellow
                        }
                    }
                }
                else {
                    # Single-line raw checksum file (.sha256 / .sha512)
                    # Matches MD5 (32), SHA1 (40), SHA256 (64), or SHA512 (128) hex strings
                    if ($FetchedData -match "([a-fA-F0-9]{32,128})") {
                        $ExpectedHash = $Matches[1]
                    }
                }
            }
        }
        catch {
            Write-Host "     [?] Remote hash lookup failed: $_" -ForegroundColor Yellow
        }
    }

    # -- Verify we have a hash to compare against --
    if ([string]::IsNullOrEmpty($ExpectedHash)) {
        Write-Host "  [FAIL] No valid hash found. Verification aborted." -ForegroundColor Red
        return $null
    }

    if (-not (Test-Path $FilePath)) {
        Write-Host "  [FAIL] File not found: $FilePath" -ForegroundColor Red
        return $null
    }

    # -- Compute and compare --
    $ComputedHash = (Get-FileHashDotNet -Path $FilePath -Algorithm $Algorithm).Trim()
    $TargetCleanHash = $ExpectedHash.ToLower().Trim()

    if ($ComputedHash -eq $TargetCleanHash) {
        Write-Host "  [OK] $Algorithm verified ($ComputedHash)" -ForegroundColor Green
        return $ComputedHash
    }
    else {
        Write-Host "  [FAIL] HASH MISMATCH!" -ForegroundColor Red
        Write-Host "     Computed: $ComputedHash" -ForegroundColor Red
        Write-Host "     Expected: $TargetCleanHash" -ForegroundColor Yellow
        return $false
    }
}


function Test-GpgSignature {
    param (
        [string]$TargetFile,
        [string]$SigUrl,
        [string]$GpgKey,
        [string]$GpgServer,
        [object]$HttpClient
    )

    # GPG is optional -- skip gracefully if not installed
    if (-not (Get-Command gpg -ErrorAction SilentlyContinue)) {
        Write-Host "  [?] GPG not found on PATH. Skipping signature verification." -ForegroundColor Yellow
        Write-Host "     Tip: Git for Windows includes gpg.exe -- add Git\usr\bin to your PATH." -ForegroundColor DarkGray
        return $true
    }

    if ([string]::IsNullOrEmpty($SigUrl) -or [string]::IsNullOrEmpty($GpgKey)) {
        Write-Host "  [i] No GPG signature configured for this target." -ForegroundColor DarkGray
        return $true
    }

    $SigFile = "$TargetFile.sig"
    Write-Host "  [GPG] Verifying signature..." -ForegroundColor Yellow

    try {
        # Import the distro signing key from the configured keyserver
        Write-Host "     [>] Fetching key [$GpgKey] from $GpgServer" -ForegroundColor DarkGray
        gpg --keyserver $GpgServer --recv-keys $GpgKey 2>$null | Out-Null

        # Download the detached signature file
        $SigBytes = $HttpClient.GetByteArrayAsync($SigUrl).GetAwaiter().GetResult()
        [System.IO.File]::WriteAllBytes($SigFile, $SigBytes)

        # Verify the ISO against the signature
        $GpgOutput = gpg --verify $SigFile $TargetFile 2>&1

        # Clean up the temporary .sig file
        Remove-Item $SigFile -Force -ErrorAction SilentlyContinue

        if ($GpgOutput -match "Good signature") {
            Write-Host "  [OK] GPG signature verified -- authentic." -ForegroundColor Green
            return $true
        }
        else {
            Write-Host "  [FAIL] GPG SIGNATURE REJECTED!" -ForegroundColor Red
            Write-Host "     $GpgOutput" -ForegroundColor DarkGray
            return $false
        }
    }
    catch {
        Write-Host "  [FAIL] GPG verification error: $_" -ForegroundColor Red
        if (Test-Path $SigFile) { Remove-Item $SigFile -Force }
        return $false
    }
}


function Get-HmacSha256 {
    param (
        [string]$Data,
        [string]$HexKey
    )
    $KeyBytes = [System.Text.Encoding]::UTF8.GetBytes($HexKey)
    $DataBytes = [System.Text.Encoding]::UTF8.GetBytes($Data)
    
    $Hmac = [System.Security.Cryptography.HMACSHA256]::new($KeyBytes)
    $HmacBytes = $Hmac.ComputeHash($DataBytes)
    $Hmac.Dispose()
    
    $Hex = [System.BitConverter]::ToString($HmacBytes) -replace '-'
    return $Hex.ToLower()
}

function Get-OrCreateHmacKey {
    param (
        [string]$KeyFilePath
    )

    if (Test-Path $KeyFilePath) {
        try {
            $Encrypted = (Get-Content $KeyFilePath -Raw).Trim()
            $Credential = New-Object System.Management.Automation.PSCredential("key", (ConvertTo-SecureString $Encrypted -ErrorAction Stop))
            $Key = $Credential.GetNetworkCredential().Password
            if ($Key.Length -eq 64) {
                return $Key
            }
        }
        catch {
            # Catch decryption failures if machine ID or user context changed
        }
    }

    # Generate a random 32-byte hex key (256-bit entropy)
    $RandBytes = [System.Byte[]]::new(32)
    $Rng = [System.Security.Cryptography.RNGCryptoServiceProvider]::new()
    $Rng.GetBytes($RandBytes)
    $Rng.Dispose()

    $HexKey = ([System.BitConverter]::ToString($RandBytes) -replace '-').ToLower()

    try {
        $SecureKey = ConvertTo-SecureString -String $HexKey -AsPlainText -Force
        $Encrypted = ConvertFrom-SecureString $SecureKey
        $Encrypted | Set-Content $KeyFilePath -Force
    }
    catch {
        # Fallback to plain text if DPAPI is unsupported in this container environment
        $HexKey | Set-Content $KeyFilePath -Force
    }

    return $HexKey
}

function Get-PinnedTrustStore {
    $DataDir = Join-Path $PSScriptRoot "..\data"
    $TrustFile = Join-Path $DataDir ".castle_trust.json"
    $SigFile = Join-Path $DataDir ".castle_trust.sig"

    $HomeDir = if ($env:USERPROFILE) { $env:USERPROFILE } else { $env:HOME }
    $KeyFile = Join-Path $HomeDir ".castle_vm_key"

    if (Test-Path $TrustFile) {
        $JsonData = (Get-Content $TrustFile -Raw).Trim()
        $Key = Get-OrCreateHmacKey -KeyFilePath $KeyFile
        $CalculatedSig = Get-HmacSha256 -Data $JsonData -HexKey $Key

        $StoredSig = ""
        if (Test-Path $SigFile) {
            $StoredSig = (Get-Content $SigFile -Raw).Trim().ToLower()
        }

        if ($CalculatedSig -eq $StoredSig) {
            $Obj = $JsonData | ConvertFrom-Json
            $Hash = @{}
            if ($Obj) {
                foreach ($Prop in $Obj.psobject.Properties) {
                    $Hash[$Prop.Name] = $Prop.Value
                }
            }
            return $Hash
        }
        else {
            Write-Host "  [SECURITY WARNING] Local trust store signature verification failed!" -ForegroundColor Red
            Write-Host "     The trust store has been tampered with or is corrupted. Ignoring local cache." -ForegroundColor Yellow
            if (Test-Path $TrustFile) { Remove-Item $TrustFile -Force }
            if (Test-Path $SigFile) { Remove-Item $SigFile -Force }
        }
    }
    return @{}
}

function Save-PinnedTrustStore {
    param ([hashtable]$TrustStore)
    $DataDir = Join-Path $PSScriptRoot "..\data"
    $TrustFile = Join-Path $DataDir ".castle_trust.json"
    $SigFile = Join-Path $DataDir ".castle_trust.sig"

    $HomeDir = if ($env:USERPROFILE) { $env:USERPROFILE } else { $env:HOME }
    $KeyFile = Join-Path $HomeDir ".castle_vm_key"

    $JsonData = ($TrustStore | ConvertTo-Json).Trim()
    $JsonData | Set-Content $TrustFile -Force

    $Key = Get-OrCreateHmacKey -KeyFilePath $KeyFile
    $Sig = Get-HmacSha256 -Data $JsonData -HexKey $Key
    $Sig | Set-Content $SigFile -Force
}


function Test-IsoIntegrity {
    <#
    .SYNOPSIS
        High-level integrity check for a single ISO target.
        Runs all applicable hash + GPG checks. Implements cryptographic Trust Pinning.
    #>
    param (
        [hashtable]$Target,
        [string]$FilePath,
        [object]$HttpClient
    )

    if ($Target.IsFolder) {
        Write-Host "  [i] Skipping integrity verification for folder target: $($Target.Name)" -ForegroundColor Green
        return $true
    }

    $HashMatch = $false
    $FinalHash = $null
    $IsoName = if ($Target.File) { $Target.File } else { [System.IO.Path]::GetFileName($FilePath) }

    # -- Check Pinned Trust Store --
    $TrustStore = Get-PinnedTrustStore
    if ($TrustStore.ContainsKey($IsoName)) {
        Write-Host "  [TRUST PIN] Validating against pinned local trust store..." -ForegroundColor Magenta
        $ExpectedPinnedHash = $TrustStore[$IsoName]
        $ComputedPinnedHash = (Get-FileHashDotNet -Path $FilePath -Algorithm "SHA512").Trim()
        
        if ($ComputedPinnedHash -eq $ExpectedPinnedHash) {
            Write-Host "  [OK] Trust Pinning verification passed. ISO matches pinned state." -ForegroundColor Green
            return $true
        }
        else {
            Write-Host "  [FAIL] TRUST PIN MISMATCH! The cached ISO differs from the initially trusted version." -ForegroundColor Red
            return $false
        }
    }

    Write-Host "  [i] No pinned hash found. Running remote integrity checks..." -ForegroundColor DarkGray

    # Both layers run cumulatively: a target can carry a static/pinned hash AND
    # remote manifest URLs (e.g. GitHubAsset targets pair the API asset digest
    # with upstream .sha256/.md5 files), and every configured check must pass.
    $HashMatch = $true
    $HashChecksRun = 0

    # -- Layer A: Static hash (Windows ISOs, macOS shim, GitHub asset digests) --
    if ($Target.ExpectedHash) {
        $HashChecksRun++
        $Algo = if ($Target.HashAlgorithm) { $Target.HashAlgorithm } else { "SHA256" }
        $Result = Test-CryptographicHash `
            -FilePath $FilePath `
            -ExpectedHash $Target.ExpectedHash `
            -Algorithm $Algo `
            -HttpClient $HttpClient

        if ($Result) { $FinalHash = $Result } else { $HashMatch = $false }
    }

    # -- Layer B: Remote hash manifest URLs (multi-algorithm audit) --
    if ($HashMatch) {
        $HashKeys = @($Target.Keys) | Where-Object { $_ -like "HashUrl*" -and $_ -notlike "*Template" }

        foreach ($Key in $HashKeys) {
            $HashChecksRun++
            $KeyString = [string]$Key
            $Algo = $KeyString -replace "^HashUrl", ""
            $Url = $Target[$KeyString]

            $Result = Test-CryptographicHash `
                -FilePath $FilePath `
                -HashUrl $Url `
                -Algorithm $Algo `
                -IsoName $Target.IsoName `
                -HttpClient $HttpClient

            if ($Result) {
                $FinalHash = $Result
            }
            else {
                $HashMatch = $false
                break
            }
        }
    }

    if ($HashChecksRun -eq 0) {
        Write-Host "  [?] No hash verification data for this target. Proceeding unverified." -ForegroundColor Yellow
    }

    # -- GPG signature layer (runs only if hash passed) --
    if ($HashMatch -and $Target.SigUrl) {
        $GpgResult = Test-GpgSignature `
            -TargetFile $FilePath `
            -SigUrl $Target.SigUrl `
            -GpgKey $Target.GpgKey `
            -GpgServer $Target.GpgServer `
            -HttpClient $HttpClient

        if (-not $GpgResult) { return $false }
    }

    # -- Pin Verified Hash if we successfully verified remotely --
    if ($HashMatch -and $FinalHash) {
        Write-Host "  [TRUST PIN] Pinning verified hash to local trust store." -ForegroundColor Magenta
        $PinnedHashToSave = (Get-FileHashDotNet -Path $FilePath -Algorithm "SHA512").Trim()
        $TrustStore[$IsoName] = $PinnedHashToSave
        Save-PinnedTrustStore -TrustStore $TrustStore
    }

    return $HashMatch
}

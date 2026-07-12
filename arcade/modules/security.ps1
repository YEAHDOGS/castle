# ==============================================================================
# Hashing & Cryptographic Security Library Module
# ==============================================================================

function Get-FileHashDotNet {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Path,
        [string]$Algorithm = "SHA256"
    )

    if (-not (Test-Path $Path)) {
        throw "File not found: $Path"
    }

    $stream = [System.IO.File]::OpenRead($Path)
    $hasher = $null
    try {
        $AlgoUpper = $Algorithm.ToUpper()
        if ($AlgoUpper -eq "SHA256") { $hasher = [System.Security.Cryptography.SHA256]::Create() }
        elseif ($AlgoUpper -eq "MD5") { $hasher = [System.Security.Cryptography.MD5]::Create() }
        elseif ($AlgoUpper -eq "SHA1") { $hasher = [System.Security.Cryptography.SHA1]::Create() }
        elseif ($AlgoUpper -eq "SHA512") { $hasher = [System.Security.Cryptography.SHA512]::Create() }
        elseif ($AlgoUpper -eq "SHA384") { $hasher = [System.Security.Cryptography.SHA384]::Create() }
        else { $hasher = [System.Security.Cryptography.SHA256]::Create() }

        $hashBytes = $hasher.ComputeHash($stream)
        $hex = ([System.BitConverter]::ToString($hashBytes) -replace '-').ToLower()
        return $hex
    }
    finally {
        if ($stream) { $stream.Close(); $stream.Dispose() }
        if ($hasher) { $hasher.Dispose() }
    }
}

function Test-FileHash {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Path,
        [Parameter(Mandatory=$true)]
        [string]$ExpectedHash,
        [string]$Algorithm = "SHA256"
    )

    Write-Host "  [SECURITY] Verifying $Algorithm hash of: $(Split-Path $Path -Leaf)..." -ForegroundColor Yellow
    
    if (-not (Test-Path $Path)) {
        Write-Host "  [FAIL] File does not exist for validation: $Path" -ForegroundColor Red
        return $false
    }

    $Computed = Get-FileHashDotNet -Path $Path -Algorithm $Algorithm
    $CleanExpected = $ExpectedHash.Trim().ToLower()

    if ($Computed -eq $CleanExpected) {
        Write-Host "  [OK] Hash matches: $Computed" -ForegroundColor Green
        return $true
    }
    else {
        Write-Host "  [FAIL] HASH MISMATCH!" -ForegroundColor Red
        Write-Host "     Computed: $Computed" -ForegroundColor Red
        Write-Host "     Expected: $CleanExpected" -ForegroundColor Yellow
        return $false
    }
}

# ==============================================================================
# HMAC CRYPTOGRAPHY (Hash-based Message Authentication Codes)
# ==============================================================================

function Get-HmacSha256 {
    param (
        [Parameter(Mandatory=$true)]
        [string]$Data,
        [Parameter(Mandatory=$true)]
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

function Get-FileHmacSha256 {
    param (
        [Parameter(Mandatory=$true)]
        [string]$Path,
        [Parameter(Mandatory=$true)]
        [string]$HexKey
    )
    if (-not (Test-Path $Path)) { throw "File not found: $Path" }
    
    $KeyBytes = [System.Text.Encoding]::UTF8.GetBytes($HexKey)
    $stream = [System.IO.File]::OpenRead($Path)
    $Hmac = $null
    try {
        $Hmac = [System.Security.Cryptography.HMACSHA256]::new($KeyBytes)
        $HmacBytes = $Hmac.ComputeHash($stream)
        return ([System.BitConverter]::ToString($HmacBytes) -replace '-').ToLower()
    }
    finally {
        $stream.Close(); $stream.Dispose()
        if ($Hmac) { $Hmac.Dispose() }
    }
}

# ==============================================================================
# AES SYMMETRIC ENCRYPTION/DECRYPTION (AES-256 with PBKDF2)
# ==============================================================================

function Protect-FileAes {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Path,
        [Parameter(Mandatory=$true)]
        [string]$Password,
        [string]$OutputPath
    )

    if (-not (Test-Path $Path)) { throw "File not found: $Path" }
    if (-not $OutputPath) { $OutputPath = "$Path.aes" }

    # Generate random 16-byte salt (128-bit)
    $Salt = [System.Byte[]]::new(16)
    $Rng = [System.Security.Cryptography.RNGCryptoServiceProvider]::new()
    $Rng.GetBytes($Salt)
    $Rng.Dispose()

    # Derive key and IV via PBKDF2 with 10,000 iterations
    $Deriver = [System.Security.Cryptography.Rfc2898DeriveBytes]::new($Password, $Salt, 10000)
    $Key = $Deriver.GetBytes(32) # AES-256 Key
    $IV = $Deriver.GetBytes(16)  # AES block size IV
    $Deriver.Dispose()

    $Aes = [System.Security.Cryptography.Aes]::Create()
    $Aes.Key = $Key
    $Aes.IV = $IV

    $InStream = [System.IO.File]::OpenRead($Path)
    $OutStream = [System.IO.File]::Create($OutputPath)

    # Write the salt prefix to Output first
    $OutStream.Write($Salt, 0, $Salt.Length)

    $Encryptor = $Aes.CreateEncryptor()
    $CryptoStream = [System.Security.Cryptography.CryptoStream]::new($OutStream, $Encryptor, [System.Security.Cryptography.CryptoStreamMode]::Write)

    try {
        $InStream.CopyTo($CryptoStream)
        $CryptoStream.FlushFinalBlock()
        Write-Host "  [SECURITY] File encrypted successfully with AES-256: $OutputPath" -ForegroundColor Green
        return $OutputPath
    }
    finally {
        $CryptoStream.Dispose()
        $Encryptor.Dispose()
        $Aes.Dispose()
        $InStream.Close(); $InStream.Dispose()
        $OutStream.Close(); $OutStream.Dispose()
    }
}

function Unprotect-FileAes {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Path,
        [Parameter(Mandatory=$true)]
        [string]$Password,
        [string]$OutputPath
    )

    if (-not (Test-Path $Path)) { throw "File not found: $Path" }
    if (-not $OutputPath) { $OutputPath = $Path -replace '\.aes$', '' }

    $InStream = [System.IO.File]::OpenRead($Path)

    # Read the 16-byte salt
    $Salt = [System.Byte[]]::new(16)
    $BytesRead = $InStream.Read($Salt, 0, 16)
    if ($BytesRead -lt 16) {
        $InStream.Close(); $InStream.Dispose()
        throw "Invalid encrypted file: file size too small."
    }

    # Derive identical key/IV from the read salt
    $Deriver = [System.Security.Cryptography.Rfc2898DeriveBytes]::new($Password, $Salt, 10000)
    $Key = $Deriver.GetBytes(32)
    $IV = $Deriver.GetBytes(16)
    $Deriver.Dispose()

    $Aes = [System.Security.Cryptography.Aes]::Create()
    $Aes.Key = $Key
    $Aes.IV = $IV

    $OutStream = [System.IO.File]::Create($OutputPath)
    $Decryptor = $Aes.CreateDecryptor()
    $CryptoStream = [System.Security.Cryptography.CryptoStream]::new($InStream, $Decryptor, [System.Security.Cryptography.CryptoStreamMode]::Read)

    try {
        $CryptoStream.CopyTo($OutStream)
        Write-Host "  [SECURITY] File decrypted successfully with AES-256: $OutputPath" -ForegroundColor Green
        return $OutputPath
    }
    catch {
        Write-Host "  [FAIL] AES Decryption failed (invalid password or corrupted data): $_" -ForegroundColor Red
        if (Test-Path $OutputPath) { Remove-Item $OutputPath -Force }
        return $null
    }
    finally {
        $CryptoStream.Dispose()
        $Decryptor.Dispose()
        $Aes.Dispose()
        $InStream.Close(); $InStream.Dispose()
        $OutStream.Close(); $OutStream.Dispose()
    }
}

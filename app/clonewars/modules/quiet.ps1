# ==============================================================================
# QUIET -- run a command-line tool without a console window popping up
# ==============================================================================
# The Castle box starts these scripts with no console of their own (so nothing
# shows on the desktop). A console program started from there the usual way
# (& docker ...) then opens a console window of its own, once per call: a
# launcher that polls docker every two seconds fills the screen with them.
# Invoke-Quiet starts the program with CreateNoWindow and reads its output
# instead, which behaves the same from a terminal and from the box.

function Invoke-Quiet {
    <#
    .SYNOPSIS
        Runs a program without a window. Sets $LASTEXITCODE like & would.
    .PARAMETER Stream
        Print its output (stderr and stdout) as it comes, each line indented by -Prefix.
    .PARAMETER InputText
        Text written to its standard input, which is then closed.
    .OUTPUTS
        [pscustomobject] ExitCode, Out (stdout text), Err (stderr text), Lines (stdout lines)
    #>
    param (
        [Parameter(Mandatory, Position = 0)][string]$FilePath,
        [Parameter(Position = 1)][string[]]$ArgumentList = @(),
        [switch]$Stream,
        [string]$Prefix = "     ",
        [string]$InputText
    )
    $Psi = [System.Diagnostics.ProcessStartInfo]::new()
    $Psi.FileName = $FilePath
    foreach ($A in $ArgumentList) { [void]$Psi.ArgumentList.Add([string]$A) }
    $Psi.UseShellExecute = $false
    $Psi.CreateNoWindow = $true
    $Psi.RedirectStandardOutput = $true
    $Psi.RedirectStandardError = $true
    $Psi.RedirectStandardInput = $PSBoundParameters.ContainsKey("InputText")
    try { $P = [System.Diagnostics.Process]::Start($Psi) }
    catch {
        $global:LASTEXITCODE = 127
        return [pscustomobject]@{ ExitCode = 127; Out = ""; Err = $_.Exception.Message; Lines = @() }
    }
    if ($Psi.RedirectStandardInput) {
        $P.StandardInput.Write($InputText)
        $P.StandardInput.Close()
    }
    if ($Stream) {
        # Build tools write their progress to stderr: show it line by line, collect stdout.
        $OutTask = $P.StandardOutput.ReadToEndAsync()
        while ($null -ne ($Line = $P.StandardError.ReadLine())) { Write-Host "$Prefix$Line" }
        $Out = $OutTask.GetAwaiter().GetResult()
        foreach ($L in ($Out -split "`r?`n")) { if ($L) { Write-Host "$Prefix$L" } }
        $Err = ""
    }
    else {
        $ErrTask = $P.StandardError.ReadToEndAsync()
        $Out = $P.StandardOutput.ReadToEnd()
        $Err = $ErrTask.GetAwaiter().GetResult()
    }
    $P.WaitForExit()
    $global:LASTEXITCODE = $P.ExitCode
    return [pscustomobject]@{ ExitCode = $P.ExitCode; Out = $Out; Err = $Err; Lines = @($Out -split "`r?`n" | Where-Object { $_ -ne "" }) }
}

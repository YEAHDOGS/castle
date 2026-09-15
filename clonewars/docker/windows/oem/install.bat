@echo off
rem ==============================================================================
rem Castle -- runs once at the end of the unattended Windows install (dockur /oem)
rem ==============================================================================
rem dockur/windows copies /oem to C:\OEM and runs this file as the last setup
rem step. Every .exe from clonewars\scripts\wine-apps rides along in C:\OEM\apps.
rem Installers get a silent flag where one is known; anything else runs as-is
rem and may show a wizard in the web viewer. A log is left at C:\OEM\install.log.
setlocal enabledelayedexpansion
set "APPS=%~dp0apps"
set "LOG=%~dp0install.log"
echo [%date% %time%] castle install.bat start > "%LOG%"

if not exist "%APPS%\*.exe" (
    echo no .exe files in %APPS% >> "%LOG%"
    goto :eof
)

for %%F in ("%APPS%\*.exe") do (
    set "NAME=%%~nxF"
    set "FLAGS="
    rem -- known silent switches ------------------------------------------------
    if /I "!NAME!"=="DiscordSetup.exe"        set "FLAGS=-s"
    if /I "!NAME!"=="Install Mullvad VPN.exe" set "FLAGS=/S"
    rem putty.exe is portable: copy it to the desktop instead of running it.
    if /I "!NAME!"=="putty.exe" (
        copy /Y "%%F" "%PUBLIC%\Desktop\" >> "%LOG%" 2>&1
        echo copied !NAME! to the public desktop >> "%LOG%"
    ) else (
        echo running !NAME! !FLAGS! >> "%LOG%"
        start "" /wait "%%F" !FLAGS! >> "%LOG%" 2>&1
        echo   exit code !errorlevel! >> "%LOG%"
    )
)
echo [%date% %time%] castle install.bat done >> "%LOG%"
endlocal

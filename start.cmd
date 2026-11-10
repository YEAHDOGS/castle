@echo off
rem start.cmd -- runs start.sh on Windows from PowerShell or Command Prompt.
rem Windows cannot run a .sh file by itself, so this finds a bash that can and hands
rem start.sh to it with the same options:   .\start.cmd --browser
rem
rem No install folder is assumed. Every bash this terminal can already reach is tried:
rem   1. the one named in CASTLE_BASH, if you set it
rem   2. every bash on PATH
rem   3. the bash that ships beside every git on PATH
rem and each is tested by running it. The first that is a Windows bash with the tools
rem start.sh uses wins. WSL's bash (Linux) and a bash without those tools are passed over.
setlocal

set "BASH="
if defined CASTLE_BASH call :try "%CASTLE_BASH%"
for /f "delims=" %%B in ('where bash 2^>nul') do call :try "%%B"
for /f "delims=" %%G in ('where git 2^>nul') do (
    call :try "%%~dpGbash.exe"
    call :try "%%~dpG..\bin\bash.exe"
    call :try "%%~dpG..\..\bin\bash.exe"
)
if not defined BASH (
    echo [castle] No usable bash was found. start.sh needs a Windows bash ^(Git Bash or MSYS2^) that has
    echo [castle] dirname, sed, find, grep, tr and cygpath. Put one on PATH, or point CASTLE_BASH at it:
    echo [castle]     set CASTLE_BASH=C:\path\to\bash.exe
    exit /b 1
)

"%BASH%" "%~dp0start.sh" %*
exit /b %ERRORLEVEL%

rem Keeps the first candidate that exists, is a Windows bash, and can find the tools.
:try
if defined BASH exit /b 0
if not exist "%~1" exit /b 0
"%~1" -c "case $(uname -s) in MINGW*|MSYS*|CYGWIN*) ;; *) exit 1 ;; esac; command -v dirname sed find grep tr cygpath >/dev/null" >nul 2>&1
if not errorlevel 1 set "BASH=%~1"
exit /b 0

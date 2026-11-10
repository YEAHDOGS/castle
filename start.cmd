@echo off
rem start.cmd -- runs start.sh on Windows from PowerShell or Command Prompt.
rem Windows cannot run a .sh file by itself, and "bash" there usually means WSL (Linux),
rem which has neither your Windows Rust nor the desktop app. This finds Git Bash and
rem hands start.sh to it with the same options:   .\start.cmd --browser
setlocal

rem A usable Git Bash has the standard tools (dirname, sed, ...) next to it in usr\bin.
rem A bare bash.exe without them is skipped.
set "BASH="
for /f "delims=" %%G in ('where git 2^>nul') do call :try "%%~dpG.."
call :try "%ProgramFiles%\Git"
call :try "%LocalAppData%\Programs\Git"
if not defined BASH (
    echo [castle] Git Bash was not found. Install Git for Windows from https://git-scm.com/download/win
    exit /b 1
)

"%BASH%" "%~dp0start.sh" %*
exit /b %ERRORLEVEL%

:try
if defined BASH exit /b 0
if exist "%~1\bin\bash.exe" if exist "%~1\usr\bin\dirname.exe" set "BASH=%~1\bin\bash.exe"
exit /b 0

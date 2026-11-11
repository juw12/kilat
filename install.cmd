@echo off
rem Kilat installer for Windows.
rem Copies kilat.exe into your own Programs folder and adds that folder to your PATH.
rem It needs no administrator rights and changes nothing else.
rem
rem   install.cmd            install, then wait for a key
rem   install.cmd /quiet     install without waiting
rem   install.cmd /dry-run   only show what would happen
setlocal
set "KILAT_HOME=%~dp0"
set "KILAT_INSTALL_MODE=install"
set "KILAT_QUIET="
:next_argument
if "%~1"=="" goto arguments_done
if /i "%~1"=="/dry-run" set "KILAT_INSTALL_MODE=dry-run"
if /i "%~1"=="/quiet" set "KILAT_QUIET=1"
shift /1
goto next_argument
:arguments_done
set "KILAT_SOURCE=%KILAT_HOME%target\release\kilat.exe"
if not exist "%KILAT_SOURCE%" set "KILAT_SOURCE=%KILAT_HOME%kilat.exe"
if not exist "%KILAT_SOURCE%" goto no_exe
set "KILAT_INSTALL_DIR=%LOCALAPPDATA%\Programs\Kilat"
echo From: "%KILAT_SOURCE%"
echo To:   "%KILAT_INSTALL_DIR%\kilat.exe"
if "%KILAT_INSTALL_MODE%"=="dry-run" goto update_path
if not exist "%KILAT_INSTALL_DIR%" mkdir "%KILAT_INSTALL_DIR%"
copy /y "%KILAT_SOURCE%" "%KILAT_INSTALL_DIR%\kilat.exe" >nul
if errorlevel 1 goto copy_failed
:update_path
powershell -NoProfile -Command "$dir = $env:KILAT_INSTALL_DIR.TrimEnd('\'); $raw = (Get-Item 'HKCU:\Environment').GetValue('Path', '', 'DoNotExpandEnvironmentNames'); $parts = @($raw -split ';' | Where-Object { $_ -ne '' }); $known = @($parts | ForEach-Object { [Environment]::ExpandEnvironmentVariables($_).TrimEnd('\') }); if ($known -contains $dir) { Write-Output 'Your PATH already contains the Kilat folder.' } elseif ($env:KILAT_INSTALL_MODE -eq 'dry-run') { Write-Output ('Your PATH would get the Kilat folder added at the end. It has ' + $parts.Count + ' entries now.') } else { Set-ItemProperty -Path 'HKCU:\Environment' -Name 'Path' -Value (($parts + $dir) -join ';') -Type ExpandString; [Environment]::SetEnvironmentVariable('KILAT_PATH_REFRESH', $null, 'User'); Write-Output 'Added the Kilat folder to your PATH.' }"
if errorlevel 1 goto path_failed
echo.
if "%KILAT_INSTALL_MODE%"=="dry-run" echo Dry run: nothing was changed.
if "%KILAT_INSTALL_MODE%"=="install" echo Kilat is installed. Open a NEW terminal and type:  kilat version
set KILAT_EXIT=0
goto finish
:no_exe
echo kilat.exe was not found next to this installer.
echo Build it first:  python build.py
set KILAT_EXIT=1
goto finish
:copy_failed
echo Could not copy kilat.exe. Close any running Kilat program and try again.
set KILAT_EXIT=1
goto finish
:path_failed
echo kilat.exe was copied, but your PATH could not be changed.
echo Add this folder to your PATH by hand: "%KILAT_INSTALL_DIR%"
set KILAT_EXIT=1
:finish
if not defined KILAT_QUIET if "%KILAT_INSTALL_MODE%"=="install" pause
exit /b %KILAT_EXIT%

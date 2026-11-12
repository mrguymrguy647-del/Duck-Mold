@echo off
rem ================================================================
rem  Double-click this file to build and start Duck Mold Studio.
rem  The first time takes a few minutes: Rust downloads and builds
rem  the libraries it uses. After that it starts in seconds.
rem ================================================================
cd /d "%~dp0"

where cargo >nul 2>nul
if errorlevel 1 (
    echo Rust is not installed yet.
    echo Install it from https://rustup.rs and then double-click this file again.
    echo If you just installed it, restart your PC first.
    echo.
    pause
    exit /b 1
)

rem Windows can't replace DuckMoldStudio.exe while it is open.
tasklist /fi "imagename eq DuckMoldStudio.exe" | find /i "DuckMoldStudio.exe" >nul
if not errorlevel 1 (
    echo Duck Mold Studio is already open. Close it first, then double-click this file again.
    echo.
    pause
    exit /b 1
)

echo Building Duck Mold Studio... (the first build takes a few minutes)
echo.
cargo build --release
if errorlevel 1 (
    echo.
    echo Something went wrong while building.
    echo Take a screenshot of this window and send it to Claude.
    echo.
    pause
    exit /b 1
)

start "" "target\release\DuckMoldStudio.exe"

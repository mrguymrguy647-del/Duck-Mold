@echo off
rem ================================================================
rem  Double-click this file to put a "Duck Mold Studio" shortcut on
rem  your desktop. Run "Start Duck Mold Studio.bat" at least once
rem  first, so the Studio is built. The shortcut always opens the
rem  newest version you've built.
rem ================================================================
cd /d "%~dp0"

if not exist "target\release\DuckMoldStudio.exe" (
    echo Duck Mold Studio hasn't been built yet.
    echo Double-click "Start Duck Mold Studio.bat" first, then double-click this file again.
    echo.
    pause
    exit /b 1
)

powershell -NoProfile -Command "$s = (New-Object -ComObject WScript.Shell).CreateShortcut([Environment]::GetFolderPath('Desktop') + '\Duck Mold Studio.lnk'); $s.TargetPath = '%~dp0target\release\DuckMoldStudio.exe'; $s.WorkingDirectory = '%~dp0'; $s.IconLocation = '%~dp0target\release\DuckMoldStudio.exe,0'; $s.Save()"
if errorlevel 1 (
    echo.
    echo Could not create the shortcut. Take a screenshot of this window and send it to Claude.
) else (
    echo.
    echo Done! There is now a "Duck Mold Studio" shortcut on your desktop.
    echo Tip: right-click it and choose "Pin to taskbar" if you like.
    echo You can also drop a recipe file on the shortcut to open it.
)
echo.
pause

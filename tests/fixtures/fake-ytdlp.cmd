@echo off
setlocal
for %%a in (%*) do set last=%%a
set last=%last:"=%
for %%a in (%last:/= %) do set id=%%a
echo %id%>> "%~dp0calls.txt"
if "%id%"=="slow" ping -n 4 127.0.0.1 >nul
if "%id%"=="fail" (echo ERROR: extracteur en panne 1>&2 & exit /b 1)
echo data> %id%.mp4
set cwd=%cd:\=/%
echo {"id":"%id%","title":"titre %id%","duration":5,"filepath":"%cwd%/%id%.mp4"}

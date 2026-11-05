@echo off
setlocal
echo %* | findstr /c:"volumedetect" >nul && (echo max_volume: -5.0 dB 1>&2 & exit /b 0)
for %%a in (%*) do set last=%%a
set last=%last:"=%
echo x> "%last%"
echo %* | findstr /c:"badmp3" >nul && (echo conversion impossible 1>&2 & exit /b 1)
exit /b 0

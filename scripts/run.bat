@echo off
setlocal EnableDelayedExpansion
chcp 65001 >nul 2>&1

echo.
echo ========================================
echo   Smart Speaker Launcher
echo ========================================
echo.

:: Application root directory (same location as run.bat)
set "APP_DIR=%~dp0"

:: Detect variant and ensure CUDA DLLs are accessible
set "VARIANT=cpu"
if exist "%APP_DIR%.variant" set /p VARIANT=<"%APP_DIR%.variant"

if /i "%VARIANT%"=="cuda" (
    :: Check NVIDIA driver version (CUDA 12.8 requires driver >= 570)
    for /f "tokens=*" %%V in ('nvidia-smi --query-gpu=driver_version --format=csv,noheader 2^>nul') do set "DRIVER_VER=%%V"
    if defined DRIVER_VER (
        for /f "tokens=1 delims=." %%M in ("!DRIVER_VER!") do set "DRIVER_MAJOR=%%M"
        echo       NVIDIA Driver: !DRIVER_VER!
        if !DRIVER_MAJOR! LSS 570 (
            echo.
            echo  *** WARNING: NVIDIA driver !DRIVER_VER! is too old for CUDA 12.8 ***
            echo  *** Please update to driver 570.x or later.                     ***
            echo  *** Download: https://www.nvidia.com/drivers                     ***
            echo.
            pause
            exit /b 1
        )
    ) else (
        echo       WARNING: nvidia-smi not found. Cannot verify GPU driver.
    )

    if not exist "%APP_DIR%cublas64_12.dll" (
        :: DLLs not bundled - add CUDA Toolkit to PATH
        for %%P in (
            "%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v12.8\bin"
            "%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v12.6\bin"
            "%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v12.4\bin"
        ) do (
            if exist "%%~P\cublas64_12.dll" set "PATH=%%~P;%PATH%"
        )
    )
)

:: Find Ollama
set OLLAMA_CMD=
where ollama >nul 2>&1
if %errorlevel% equ 0 (
    set OLLAMA_CMD=ollama
) else if exist "%LOCALAPPDATA%\Programs\Ollama\ollama.exe" (
    set "OLLAMA_CMD=%LOCALAPPDATA%\Programs\Ollama\ollama.exe"
) else (
    echo ERROR: Ollama not found.
    echo Please run setup.bat first.
    pause
    exit /b 1
)

:: Find VOICEVOX
set VOICEVOX_CMD=
if exist "%LOCALAPPDATA%\Programs\VOICEVOX\VOICEVOX.exe" (
    set "VOICEVOX_CMD=%LOCALAPPDATA%\Programs\VOICEVOX\VOICEVOX.exe"
) else if exist "%ProgramFiles%\VOICEVOX\VOICEVOX.exe" (
    set "VOICEVOX_CMD=%ProgramFiles%\VOICEVOX\VOICEVOX.exe"
)

:: ----------------------------------------
:: [1/3] Check Ollama
:: ----------------------------------------
echo [1/3] Checking Ollama...
curl -s http://localhost:11434/api/tags >nul 2>&1
if %errorlevel% equ 0 (
    echo       Ollama: Running
) else (
    echo       Starting Ollama...
    start "" "%OLLAMA_CMD%" serve
    timeout /t 3 /nobreak >nul
)

:: ----------------------------------------
:: [2/3] Check VOICEVOX
:: ----------------------------------------
echo [2/3] Checking VOICEVOX...
curl -s http://localhost:50021/version >nul 2>&1
if %errorlevel% equ 0 (
    echo       VOICEVOX: Running
    goto voicevox_done
)
if not defined VOICEVOX_CMD (
    echo       WARNING: VOICEVOX not found.
    echo       Please start VOICEVOX manually.
    goto voicevox_done
)
echo       Starting VOICEVOX...
start "" "%VOICEVOX_CMD%"
echo       Waiting for VOICEVOX to start...
:wait_voicevox
timeout /t 2 /nobreak >nul
curl -s http://localhost:50021/version >nul 2>&1
if %errorlevel% neq 0 goto wait_voicevox
echo       VOICEVOX: Ready
:voicevox_done

:: ----------------------------------------
:: Service status check
:: ----------------------------------------
echo.
echo   --- Service Status ---
curl -s http://localhost:11434/api/tags >nul 2>&1
if %errorlevel% equ 0 (
    echo   Ollama:   OK
) else (
    echo   Ollama:   Failed to start
    pause
    exit /b 1
)

curl -s http://localhost:50021/version >nul 2>&1
if %errorlevel% equ 0 (
    echo   VOICEVOX: OK
) else (
    echo   VOICEVOX: Failed to start
    pause
    exit /b 1
)
echo   --------------------
echo.

:: ----------------------------------------
:: [3/3] Launch Smart Speaker
:: ----------------------------------------
echo [3/3] Launching Smart Speaker...
echo.
cd /d "%APP_DIR%"
if exist "smart_speaker.exe" (
    smart_speaker.exe
) else (
    echo ERROR: smart_speaker.exe not found.
    echo Please check that smart_speaker.exe exists in "%APP_DIR%".
    pause
    exit /b 1
)

pause

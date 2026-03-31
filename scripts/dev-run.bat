@echo off
setlocal
chcp 65001 >nul 2>&1

echo ============================================
echo   Smart Speaker - Dev Launcher (CUDA)
echo ============================================
echo.

:: Navigate to project root (one level up from scripts/)
set "PROJECT_DIR=%~dp0.."
cd /d "%PROJECT_DIR%"

:: Force CUDA 12.8 for build
set "CUDA_VER=12.8"
if exist "%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v%CUDA_VER%\bin\nvcc.exe" (
    set "CUDA_PATH=%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v%CUDA_VER%"
    set "CUDAToolkit_ROOT=%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v%CUDA_VER%"
    set "PATH=%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v%CUDA_VER%\bin;%PATH%"
    echo Using CUDA Toolkit: v%CUDA_VER%
) else (
    echo WARNING: CUDA %CUDA_VER% not found. Build may use a different CUDA version.
)
echo.

:: Find Ollama
set OLLAMA_CMD=
where ollama >nul 2>&1
if %errorlevel% equ 0 (
    set OLLAMA_CMD=ollama
) else if exist "%LOCALAPPDATA%\Programs\Ollama\ollama.exe" (
    set "OLLAMA_CMD=%LOCALAPPDATA%\Programs\Ollama\ollama.exe"
) else (
    echo ERROR: Ollama not found. Run setup.bat first.
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

:: [1/3] Check Ollama
echo [1/3] Checking Ollama...
curl -s http://localhost:11434/api/tags >nul 2>&1
if %errorlevel% equ 0 (
    echo       Ollama is already running
) else (
    echo       Starting Ollama...
    start "" "%OLLAMA_CMD%" serve
    timeout /t 3 /nobreak >nul
)

:: [2/3] Check VOICEVOX
echo [2/3] Checking VOICEVOX...
curl -s http://localhost:50021/version >nul 2>&1
if %errorlevel% equ 0 (
    echo       VOICEVOX is already running
    goto voicevox_done
)
if not defined VOICEVOX_CMD (
    echo       WARNING: VOICEVOX not found. Please start manually.
    goto voicevox_done
)
echo       Starting VOICEVOX...
start "" "%VOICEVOX_CMD%"
echo       Waiting for VOICEVOX to start...
:wait_voicevox_dev
timeout /t 2 /nobreak >nul
curl -s http://localhost:50021/version >nul 2>&1
if %errorlevel% neq 0 goto wait_voicevox_dev
echo       VOICEVOX is ready
:voicevox_done

echo.

:: [3/3] Build and run with CUDA
echo [3/3] Building and running (CUDA %CUDA_VER%)...
echo.
cargo run --release --features cuda

pause

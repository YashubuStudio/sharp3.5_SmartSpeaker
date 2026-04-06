@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul 2>&1

echo ============================================
echo   Smart Speaker - Dev Launcher (CUDA)
echo ============================================
echo.

:: Navigate to project root (one level up from scripts/)
set "PROJECT_DIR=%~dp0.."
set "CUDA_ARCH_LIST=75-real;86-real;89-real;120-virtual"
cd /d "%PROJECT_DIR%"

:: Configure CUDA / LLVM / MSVC for local GPU builds
call "%~dp0cuda-env.bat"
set "CUDA_ENV_STATUS=%ERRORLEVEL%"
if "%CUDA_ENV_STATUS%"=="0" (
    if not defined CMAKE_CUDA_ARCHITECTURES set "CMAKE_CUDA_ARCHITECTURES=!CUDA_ARCH_LIST!"
    if not defined CUDAARCHS set "CUDAARCHS=!CUDA_ARCH_LIST!"
    echo Using CUDA Toolkit: v!CUDA_VER!
    echo Build temp: !TEMP!
) else (
    if "%CUDA_ENV_STATUS%"=="1" (
        echo ERROR: Supported CUDA Toolkit not found.
        echo Install CUDA 12.8, 12.6, or 12.4 and retry.
    ) else (
        echo ERROR: Visual Studio C++ build tools are not ready for CUDA builds.
        echo Install Visual Studio Build Tools 2022 with the C++ workload and retry.
    )
    pause
    exit /b 1
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

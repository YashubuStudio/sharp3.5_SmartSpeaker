@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul 2>&1

echo.
echo ========================================
echo   Smart Speaker Setup
echo ========================================
echo.

:: Detect variant
set VARIANT=cpu
if exist "%~dp0.variant" (
    set /p VARIANT=<"%~dp0.variant"
)

if /i "%VARIANT%"=="cuda" (
    echo   Variant: GPU ^(CUDA^)
) else (
    echo   Variant: CPU
)
echo.

:: Application root directory (same location as setup.bat)
set "APP_DIR=%~dp0"

:: ----------------------------------------
:: [1/6] Create directories
:: ----------------------------------------
echo [1/6] Creating directories...
if not exist "%APP_DIR%data\knowledge" mkdir "%APP_DIR%data\knowledge"
if not exist "%APP_DIR%models" mkdir "%APP_DIR%models"
echo       Done
echo.

:: ----------------------------------------
:: [2/6] CUDA Toolkit check (GPU variant only)
:: ----------------------------------------
if /i "%VARIANT%"=="cuda" (
    echo [2/6] Checking CUDA Toolkit 12.8+...

    REM Check if any CUDA 12.x DLL is accessible
    set "CUDA_OK=0"
    for %%P in (
        "%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v12.8\bin"
        "%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v12.6\bin"
        "%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v12.4\bin"
    ) do (
        if exist "%%~P\cublas64_12.dll" set "CUDA_OK=1"
    )

    REM Also check if bundled DLLs exist (cuda-bundled variant)
    if exist "%APP_DIR%cublas64_12.dll" set "CUDA_OK=1"

    if "!CUDA_OK!"=="1" (
        echo       CUDA 12.x libraries found
    ) else (
        echo.
        echo       CUDA Toolkit 12.8+ is required for the GPU variant.
        echo       Other CUDA 12.x versions ^(12.4, 12.6^) also work.
        echo.
        echo       NOTE: CUDA 13.x is NOT compatible. Please install CUDA 12.8.
        echo.
        echo       Installation steps:
        echo         1. Go to https://developer.nvidia.com/cuda-toolkit
        echo         2. Select Windows, x86_64, exe ^(local^)
        echo         3. Choose CUDA Toolkit 12.8
        echo         4. Run the express installation
        echo         5. Restart your PC after installation
        echo.
        echo       Verify with: nvcc --version
        echo.
        choice /c YN /m "Continue without CUDA? (Y=Continue / N=Abort)"
        if errorlevel 2 exit /b 1
    )
    echo.
) else (
    echo [2/6] CUDA check... Skipped (CPU variant)
    echo.
)

:: ----------------------------------------
:: [3/6] Ollama installation
:: ----------------------------------------
echo [3/6] Checking Ollama...
where ollama >nul 2>&1
if %errorlevel% equ 0 (
    echo       Ollama is already installed
    goto :ollama_done
)

echo       Ollama not found. Installing...
set "OLLAMA_INSTALLER=%TEMP%\OllamaSetup.exe"
echo       Downloading Ollama installer...
curl -L --progress-bar -o "!OLLAMA_INSTALLER!" "https://ollama.com/download/OllamaSetup.exe"
if not exist "!OLLAMA_INSTALLER!" (
    echo.
    echo       Failed to download Ollama installer.
    echo       Please install manually: https://ollama.com/
    pause
    exit /b 1
)
echo       Running installer...
start /wait "" "!OLLAMA_INSTALLER!"
del "!OLLAMA_INSTALLER!" >nul 2>&1
REM Refresh PATH to pick up newly installed ollama
set "PATH=%LOCALAPPDATA%\Programs\Ollama;%PATH%"
where ollama >nul 2>&1
if %errorlevel% equ 0 (
    echo       Ollama installed successfully
    goto :ollama_done
)
if exist "%LOCALAPPDATA%\Programs\Ollama\ollama.exe" (
    echo       Ollama installed successfully
    goto :ollama_done
)
echo.
echo       Ollama installation may not have completed.
echo       Please install manually: https://ollama.com/
pause
exit /b 1
:ollama_done
echo.

:: ----------------------------------------
:: [4/6] Download Ollama models
:: ----------------------------------------
echo [4/6] Downloading Ollama models...

:: Start Ollama if not running
curl -s http://localhost:11434/api/tags >nul 2>&1
if %errorlevel% neq 0 (
    echo       Starting Ollama...
    start "" ollama serve
    timeout /t 5 /nobreak >nul
)

echo       Downloading gemma3:4b (LLM)...
ollama pull gemma3:4b
if %errorlevel% neq 0 (
    echo       Failed to download gemma3:4b
    echo         Run manually later: ollama pull gemma3:4b
)

echo       Downloading nomic-embed-text (for RAG)...
ollama pull nomic-embed-text
if %errorlevel% neq 0 (
    echo       Failed to download nomic-embed-text
    echo         Run manually later: ollama pull nomic-embed-text
)
echo.

:: ----------------------------------------
:: [5/6] Download Whisper model
:: ----------------------------------------
echo [5/6] Checking Whisper model...
set "WHISPER_MODEL=ggml-large-v3-turbo.bin"
set "WHISPER_URL=https://huggingface.co/ggerganov/whisper.cpp/resolve/main/%WHISPER_MODEL%"
set "WHISPER_DEST=%APP_DIR%models\%WHISPER_MODEL%"

if exist "%WHISPER_DEST%" (
    echo       %WHISPER_MODEL% already exists
    goto :whisper_done
)

echo       %WHISPER_MODEL% not found. Downloading (~1.6GB^)...
echo       URL: %WHISPER_URL%
echo.
curl -L --progress-bar -o "%WHISPER_DEST%.part" "%WHISPER_URL%"
if not errorlevel 1 goto :whisper_check

echo.
echo       curl failed. Trying PowerShell...
powershell -Command "Invoke-WebRequest -Uri '%WHISPER_URL%' -OutFile '%WHISPER_DEST%.part'"

:whisper_check
if exist "%WHISPER_DEST%.part" (
    move /y "%WHISPER_DEST%.part" "%WHISPER_DEST%" >nul
    echo       Download complete: %WHISPER_MODEL%
) else (
    echo.
    echo       Failed to download Whisper model.
    echo       The app will prompt you to download on first run.
)
:whisper_done
echo.

:: ----------------------------------------
:: [6/6] VOICEVOX check
:: ----------------------------------------
echo [6/6] Checking VOICEVOX...
curl -s http://localhost:50021/version >nul 2>&1
if %errorlevel% equ 0 (
    echo       VOICEVOX is running
) else (
    echo.
    echo       VOICEVOX not found.
    echo       VOICEVOX is required for text-to-speech.
    echo.
    echo       Installation steps:
    echo         1. Download from https://voicevox.hiroshiba.jp/
    echo         2. Install and launch
    echo         3. It runs in the system tray (http://localhost:50021)
    echo.
    echo       Please make sure VOICEVOX is running when using Smart Speaker.
)
echo.

:: ----------------------------------------
:: Done
:: ----------------------------------------
echo ========================================
echo   Setup Complete
echo ========================================
echo.
echo   Next steps:
echo     1. Install and start VOICEVOX
echo     2. Run run.bat
echo.
pause

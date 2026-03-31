@echo off
setlocal enabledelayedexpansion
chcp 65001 >nul 2>&1

:: ============================================================
:: Smart Speaker Release Build Script
:: Builds CPU + CUDA variants, packages as 7z
:: Output: cpu / cuda (no DLLs) / cuda-bundled (with CUDA DLLs)
:: Requires: CUDA Toolkit 12.8 for CUDA builds
:: ============================================================

:: Navigate to project root (one level up from scripts/)
cd /d "%~dp0.."

:: Version (keep in sync with Cargo.toml)
set VERSION=0.1.0
set RELEASE_BASE=release
set CUDA_VER=12.8

:: Parse options
set DO_BUILD=1
set INCLUDE_KNOWLEDGE=0

:parse_args
if "%~1"=="" goto :done_args
if "%~1"=="--no-build" (
    set DO_BUILD=0
    shift
    goto :parse_args
)
if "%~1"=="--with-knowledge" (
    set INCLUDE_KNOWLEDGE=1
    shift
    goto :parse_args
)
if "%~1"=="--help" (
    echo Usage: release.bat [OPTIONS]
    echo.
    echo Options:
    echo   --no-build         Skip build ^(use existing binaries^)
    echo   --with-knowledge   Include data/knowledge/ contents
    echo   --help             Show this help
    echo.
    echo Creates 3 release archives:
    echo   cpu           - CPU-only build
    echo   cuda          - CUDA build ^(requires CUDA Toolkit 12.8+ on user machine^)
    echo   cuda-bundled  - CUDA build with DLLs included ^(standalone^)
    exit /b 0
)
echo Unknown option: %~1
exit /b 1
:done_args

echo.
echo ============================================================
echo  Smart Speaker Release Builder v%VERSION%
echo  CUDA Toolkit: %CUDA_VER%
echo  Output: cpu / cuda / cuda-bundled
echo ============================================================
echo.

:: Add 7-Zip to PATH if not already available
where 7z >nul 2>&1
if %errorlevel% neq 0 (
    if exist "%ProgramFiles%\7-Zip\7z.exe" (
        set "PATH=%ProgramFiles%\7-Zip;%PATH%"
    ) else if exist "%ProgramFiles(x86)%\7-Zip\7z.exe" (
        set "PATH=%ProgramFiles(x86)%\7-Zip;%PATH%"
    ) else (
        echo ERROR: 7-Zip not found.
        echo Please install 7-Zip: https://www.7-zip.org/
        pause
        exit /b 1
    )
)

:: Locate CUDA Toolkit 12.8
set "CUDA_ROOT="
set "SKIP_CUDA=0"
for %%D in (
    "%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v%CUDA_VER%"
) do (
    if exist "%%~D\bin\nvcc.exe" set "CUDA_ROOT=%%~D"
)
if not defined CUDA_ROOT (
    echo WARNING: CUDA Toolkit %CUDA_VER% not found.
    echo Expected: %ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v%CUDA_VER%
    echo CUDA variants will be skipped. Only CPU variant will be built.
    echo.
    set SKIP_CUDA=1
) else (
    echo Using CUDA Toolkit: %CUDA_ROOT%
)
echo.

if %SKIP_CUDA%==1 goto :skip_cuda_env
:: Force CMake to use CUDA 12.8 (not 13.x)
set "CUDA_PATH=%CUDA_ROOT%"
set "CUDAToolkit_ROOT=%CUDA_ROOT%"
set "CMAKE_CUDA_COMPILER=%CUDA_ROOT%\bin\nvcc.exe"
set "PATH=%CUDA_ROOT%\bin;%PATH%"
:: Fat binary: cover all modern GPU generations in a single binary
::   75 = Turing (RTX 2000), 86 = Ampere (RTX 3000), 89 = Ada (RTX 4000)
::   120-virtual = Blackwell PTX (RTX 5000+, JIT at runtime)
set "CMAKE_CUDA_ARCHITECTURES=75-real;86-real;89-real;120-virtual"
set "CUDAARCHS=75-real;86-real;89-real;120-virtual"
:skip_cuda_env

:: Create release base directory
if not exist "%RELEASE_BASE%" mkdir "%RELEASE_BASE%"

:: ========================================
:: Build both binaries
:: ========================================
if %DO_BUILD%==1 (
    echo [1/7] Building CPU variant
    cargo build --release --no-default-features
    if errorlevel 1 (
        echo.
        echo ERROR: CPU build failed
        exit /b 1
    )
    copy /y "target\release\smart_speaker.exe" "%RELEASE_BASE%\smart_speaker_cpu.exe" >nul
    echo       CPU build complete
    echo.

    if %SKIP_CUDA%==0 (
        echo [2/7] Building CUDA variant ^(CUDA %CUDA_VER%^)
        cargo build --release
        if errorlevel 1 (
            echo.
            echo ERROR: CUDA build failed
            exit /b 1
        )
        copy /y "target\release\smart_speaker.exe" "%RELEASE_BASE%\smart_speaker_cuda.exe" >nul
        echo       CUDA build complete
    ) else (
        echo [2/7] Skipping CUDA build ^(CUDA Toolkit not found^)
    )
    echo.
) else (
    echo [1/7] Skipping CPU build ^(--no-build^)
    echo [2/7] Skipping CUDA build ^(--no-build^)
    if not exist "%RELEASE_BASE%\smart_speaker_cpu.exe" (
        echo ERROR: %RELEASE_BASE%\smart_speaker_cpu.exe not found
        echo Please run without --no-build first
        exit /b 1
    )
    if %SKIP_CUDA%==0 (
        if not exist "%RELEASE_BASE%\smart_speaker_cuda.exe" (
            echo ERROR: %RELEASE_BASE%\smart_speaker_cuda.exe not found
            echo Please run without --no-build first
            exit /b 1
        )
    )
    echo.
)

:: ========================================
:: Locate CUDA DLLs for bundled variant
:: ========================================
set "CUDA_BIN="
for %%P in (
    "%CUDA_ROOT%\bin"
    "%CUDA_ROOT%\bin\x64"
) do (
    if not defined CUDA_BIN (
        if exist "%%~P\cublas64_12.dll" set "CUDA_BIN=%%~P"
    )
)

if not defined CUDA_BIN (
    echo WARNING: CUDA DLLs not found. cuda-bundled variant will be skipped.
    echo.
    set SKIP_BUNDLED=1
) else (
    echo CUDA DLLs found: %CUDA_BIN%
    echo.
    set SKIP_BUNDLED=0
)

:: ========================================
:: Package all variants
:: Variants: cpu, cuda, cuda-bundled
:: ========================================
set STEP=3
for %%V in (cpu cuda cuda-bundled) do (
    set "RELEASE_NAME=smart_speaker-v%VERSION%-%%V"
    set "RELEASE_DIR=%RELEASE_BASE%\smart_speaker-v%VERSION%-%%V"

    if "%%V"=="cpu" (
        echo [!STEP!/7] Packaging CPU variant
        set "BIN_SRC=%RELEASE_BASE%\smart_speaker_cpu.exe"
        set "PKG_VARIANT=cpu"
    )
    if "%%V"=="cuda" (
        if "!SKIP_CUDA!"=="1" (
            echo [!STEP!/7] Skipping CUDA variant ^(not built^)
            set /a STEP+=1
            echo.
            goto :skip_cuda_pkg
        )
        echo [!STEP!/7] Packaging CUDA variant
        set "BIN_SRC=%RELEASE_BASE%\smart_speaker_cuda.exe"
        set "PKG_VARIANT=cuda"
    )
    if "%%V"=="cuda-bundled" (
        if "!SKIP_CUDA!"=="1" (
            echo [!STEP!/7] Skipping CUDA-bundled variant ^(not built^)
            set /a STEP+=1
            echo.
            goto :skip_bundled_pkg
        )
        if "!SKIP_BUNDLED!"=="1" (
            echo [!STEP!/7] Skipping CUDA-bundled variant ^(DLLs not found^)
            set /a STEP+=1
            echo.
            goto :skip_bundled_pkg
        )
        echo [!STEP!/7] Packaging CUDA-bundled variant
        set "BIN_SRC=%RELEASE_BASE%\smart_speaker_cuda.exe"
        set "PKG_VARIANT=cuda"
    )

    REM Clean and create directory structure
    if exist "!RELEASE_DIR!" rmdir /s /q "!RELEASE_DIR!"
    mkdir "!RELEASE_DIR!"
    mkdir "!RELEASE_DIR!\config"
    mkdir "!RELEASE_DIR!\models"
    mkdir "!RELEASE_DIR!\data\knowledge"

    REM EXE
    echo   - smart_speaker.exe
    copy /y "!BIN_SRC!" "!RELEASE_DIR!\smart_speaker.exe" >nul

    REM CUDA DLLs (cuda-bundled only)
    if "%%V"=="cuda-bundled" (
        for %%d in (cublas64_12.dll cublasLt64_12.dll cudart64_12.dll) do (
            if exist "!CUDA_BIN!\%%d" (
                echo   - %%d
                copy /y "!CUDA_BIN!\%%d" "!RELEASE_DIR!\" >nul
            )
        )
        REM Third-party notices (required for CUDA DLL redistribution)
        if exist "scripts\THIRD_PARTY_NOTICES.txt" (
            echo   - THIRD_PARTY_NOTICES.txt
            copy /y "scripts\THIRD_PARTY_NOTICES.txt" "!RELEASE_DIR!\" >nul
        )
    )

    REM Config
    echo   - config/settings.toml
    copy /y "config\settings.toml" "!RELEASE_DIR!\config\" >nul

    REM Wake word models
    for %%f in (*.rpw) do (
        echo   - %%f
        copy /y "%%f" "!RELEASE_DIR!\" >nul
    )

    REM Variant marker
    echo   - .variant ^(!PKG_VARIANT!^)
    echo !PKG_VARIANT!> "!RELEASE_DIR!\.variant"

    REM Scripts
    echo   - run.bat
    copy /y "scripts\run.bat" "!RELEASE_DIR!\" >nul
    if exist "scripts\setup.bat" (
        echo   - setup.bat
        copy /y "scripts\setup.bat" "!RELEASE_DIR!\" >nul
    )

    REM Models README
    if exist "scripts\models_readme.txt" (
        echo   - models/README.txt
        copy /y "scripts\models_readme.txt" "!RELEASE_DIR!\models\README.txt" >nul
    )

    REM Documentation
    if exist "LICENSE" (
        echo   - LICENSE
        copy /y "LICENSE" "!RELEASE_DIR!\" >nul
    )
    if exist "README.md" (
        echo   - README.md
        copy /y "README.md" "!RELEASE_DIR!\" >nul
    )

    REM Knowledge data (optional)
    if %INCLUDE_KNOWLEDGE%==1 (
        echo   - data/knowledge/ ^(with contents^)
        if exist "data\knowledge\*" (
            xcopy /y /e /q "data\knowledge\*" "!RELEASE_DIR!\data\knowledge\" >nul 2>&1
        )
    ) else (
        echo   - data/knowledge/ ^(empty directory only^)
    )

    set /a STEP+=1
    echo.
)
:skip_cuda_pkg
:skip_bundled_pkg

:: ========================================
:: Create 7z archives
:: ========================================
set ARCHIVE_STEP=!STEP!

for %%V in (cpu cuda cuda-bundled) do (
    set "PKG_DIR=%RELEASE_BASE%\smart_speaker-v%VERSION%-%%V"
    set "PKG_7Z=%RELEASE_BASE%\smart_speaker-v%VERSION%-%%V.7z"

    if "%%V"=="cuda" if "!SKIP_CUDA!"=="1" goto :skip_cuda_7z
    if "%%V"=="cuda-bundled" if "!SKIP_CUDA!"=="1" goto :skip_bundled_7z
    if "%%V"=="cuda-bundled" if "!SKIP_BUNDLED!"=="1" goto :skip_bundled_7z

    if exist "!PKG_DIR!" (
        echo [!ARCHIVE_STEP!/7] Creating %%V archive
        if exist "!PKG_7Z!" del "!PKG_7Z!"
        7z a -mx9 "!PKG_7Z!" ".\!PKG_DIR!" >nul
        if errorlevel 1 (
            echo ERROR: Failed to create %%V archive
            exit /b 1
        )
        for %%a in ("!PKG_7Z!") do echo       Created: !PKG_7Z!  (%%~za bytes^)
        set /a ARCHIVE_STEP+=1
        echo.
    )
)
:skip_cuda_7z
:skip_bundled_7z

:: ========================================
:: Clean up temporary binaries
:: ========================================
if exist "%RELEASE_BASE%\smart_speaker_cpu.exe" del "%RELEASE_BASE%\smart_speaker_cpu.exe"
if exist "%RELEASE_BASE%\smart_speaker_cuda.exe" del "%RELEASE_BASE%\smart_speaker_cuda.exe"

:: ========================================
:: Summary
:: ========================================
echo ============================================================
echo  Release complete ^(built with CUDA %CUDA_VER%^)
echo ============================================================
echo.
echo  Archives:
for %%V in (cpu cuda cuda-bundled) do (
    set "PKG_7Z=%RELEASE_BASE%\smart_speaker-v%VERSION%-%%V.7z"
    if exist "!PKG_7Z!" (
        for %%a in ("!PKG_7Z!") do echo   %%V:  !PKG_7Z!  (%%~za bytes^)
    )
)
echo.

endlocal

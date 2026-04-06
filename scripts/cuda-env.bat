@echo off

set "CUDA_ROOT="
set "CUDA_VER="
for %%V in (12.8 12.6 12.4) do (
    if not defined CUDA_ROOT if exist "%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v%%~V\bin\nvcc.exe" (
        set "CUDA_VER=%%~V"
        set "CUDA_ROOT=%ProgramFiles%\NVIDIA GPU Computing Toolkit\CUDA\v%%~V"
    )
)

if not defined CUDA_ROOT exit /b 1

set "CUDA_PATH=%CUDA_ROOT%"
set "CUDAToolkit_ROOT=%CUDA_ROOT%"
set "CMAKE_CUDA_COMPILER=%CUDA_ROOT%\bin\nvcc.exe"
set "PATH=%CUDA_ROOT%\bin;%PATH%"

if not defined LIBCLANG_PATH (
    for %%P in (
        "%ProgramFiles%\LLVM\bin"
        "%ProgramFiles%\Microsoft Visual Studio\2022\Community\VC\Tools\Llvm\x64\bin"
        "%ProgramFiles%\Microsoft Visual Studio\2022\BuildTools\VC\Tools\Llvm\x64\bin"
        "%ProgramFiles%\Microsoft Visual Studio\2022\Professional\VC\Tools\Llvm\x64\bin"
        "%ProgramFiles%\Microsoft Visual Studio\2022\Enterprise\VC\Tools\Llvm\x64\bin"
    ) do (
        if not defined LIBCLANG_PATH if exist "%%~P\libclang.dll" set "LIBCLANG_PATH=%%~P"
    )
)

set "SMART_SPEAKER_BUILD_TEMP=%PUBLIC%\smart_speaker_cuda_temp"
if not exist "%SMART_SPEAKER_BUILD_TEMP%" mkdir "%SMART_SPEAKER_BUILD_TEMP%" >nul 2>&1
if exist "%SMART_SPEAKER_BUILD_TEMP%" (
    set "TEMP=%SMART_SPEAKER_BUILD_TEMP%"
    set "TMP=%SMART_SPEAKER_BUILD_TEMP%"
)

if not defined CUDAFLAGS set "CUDAFLAGS=-allow-unsupported-compiler"
if not defined CMAKE_CUDA_FLAGS set "CMAKE_CUDA_FLAGS=-allow-unsupported-compiler"
if not defined NVCC_APPEND_FLAGS set "NVCC_APPEND_FLAGS=-allow-unsupported-compiler"

exit /b 0

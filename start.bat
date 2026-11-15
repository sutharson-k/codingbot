@echo off
setlocal enabledelayedexpansion
title CodingBot: Autonomous Coding Agent Engine

:: Ensure Cargo/Rust binaries are on PATH for current session
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

:: Navigate to script directory
cd /d "%~dp0"

:: Check if cargo is available
where cargo >nul 2>nul
if %ERRORLEVEL% neq 0 (
    echo [ERROR] Cargo/Rust was not found in PATH or at %USERPROFILE%\.cargo\bin.
    echo Please install Rust from https://rustup.rs/
    pause
    exit /b 1
)

:: If arguments were provided, pass them directly to codingbot
if not "%~1"=="" (
    cargo run --quiet -- %*
    goto :eof
)

:MENU
cls
echo ================================================================
echo               CODINGBOT: AUTONOMOUS AGENT RUNTIME
echo ================================================================
echo.
echo   1. Run Autonomous Demo (Full Index-Plan-Code-Test-SelfCorrect-PR)
echo   2. View State Machine Graph (ASCII ^& Mermaid Topology)
echo   3. Run Coding Agent on Custom Task / Repository
echo   4. Run Test Suite (cargo test)
echo   5. Build Release Executable (cargo build --release)
echo   6. Exit
echo.
echo ================================================================
set /p CHOICE="Select an option (1-6): "

if "%CHOICE%"=="1" goto RUN_DEMO
if "%CHOICE%"=="2" goto RUN_GRAPH
if "%CHOICE%"=="3" goto RUN_CUSTOM
if "%CHOICE%"=="4" goto RUN_TESTS
if "%CHOICE%"=="5" goto RUN_BUILD
if "%CHOICE%"=="6" goto EXIT
echo.
echo [!] Invalid selection, please enter 1, 2, 3, 4, 5, or 6.
timeout /t 2 >nul
goto MENU

:RUN_DEMO
cls
echo [CodingBot] Running autonomous self-healing demo...
echo.
cargo run -- demo
echo.
echo Press any key to return to menu...
pause >nul
goto MENU

:RUN_GRAPH
cls
echo [CodingBot] Displaying LangGraph state machine topology...
echo.
cargo run -- graph
echo.
echo Press any key to return to menu...
pause >nul
goto MENU

:RUN_CUSTOM
cls
echo [CodingBot] Run Agent on Custom Repository
echo.
set /p REPO_PATH="Enter target repository path (press Enter for current folder): "
if "%REPO_PATH%"=="" set "REPO_PATH=."

set /p TASK_PROMPT="Enter task description: "
if "%TASK_PROMPT%"=="" (
    echo [ERROR] Task description cannot be empty.
    pause
    goto MENU
)

echo.
echo [CodingBot] Launching agent on "%REPO_PATH%"...
cargo run -- run --repo "%REPO_PATH%" --task "%TASK_PROMPT%" --create-branch --export-pr pr.md --export-diff changes.patch
echo.
echo Press any key to return to menu...
pause >nul
goto MENU

:RUN_TESTS
cls
echo [CodingBot] Running full test suite...
echo.
cargo test
echo.
echo Press any key to return to menu...
pause >nul
goto MENU

:RUN_BUILD
cls
echo [CodingBot] Compiling release binary...
echo.
cargo build --release
echo.
if %ERRORLEVEL% equ 0 (
    echo [SUCCESS] Binary compiled at target\release\codingbot.exe
)
echo.
echo Press any key to return to menu...
pause >nul
goto MENU

:EXIT
echo Exiting CodingBot. Goodbye!
endlocal
exit /b 0

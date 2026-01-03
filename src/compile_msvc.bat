@echo off
setlocal

:: ============================================
:: Compilation avec MSVC (Visual Studio)
:: Moins de faux positifs antivirus que MinGW
:: ============================================

set EXE_NAME=la_meuh.exe

:: Chercher vcvarsall.bat pour initialiser MSVC
set "VCVARS="
for %%v in (2022 2019 2018 2017) do (
    for %%t in (Community BuildTools Enterprise Professional) do (
        if exist "C:\Program Files\Microsoft Visual Studio\%%v\%%t\VC\Auxiliary\Build\vcvarsall.bat" (
            set "VCVARS=C:\Program Files\Microsoft Visual Studio\%%v\%%t\VC\Auxiliary\Build\vcvarsall.bat"
            goto :found
        )
        if exist "C:\Program Files (x86)\Microsoft Visual Studio\%%v\%%t\VC\Auxiliary\Build\vcvarsall.bat" (
            set "VCVARS=C:\Program Files (x86)\Microsoft Visual Studio\%%v\%%t\VC\Auxiliary\Build\vcvarsall.bat"
            goto :found
        )
    )
)
:: Chercher aussi dans les dossiers numerotes (18, 19, etc.)
for %%v in (18 19 20 21 22) do (
    if exist "C:\Program Files (x86)\Microsoft Visual Studio\%%v\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" (
        set "VCVARS=C:\Program Files (x86)\Microsoft Visual Studio\%%v\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
        goto :found
    )
    if exist "C:\Program Files\Microsoft Visual Studio\%%v\BuildTools\VC\Auxiliary\Build\vcvarsall.bat" (
        set "VCVARS=C:\Program Files\Microsoft Visual Studio\%%v\BuildTools\VC\Auxiliary\Build\vcvarsall.bat"
        goto :found
    )
)

echo Erreur: Visual Studio ou Build Tools non trouve.
echo Installe Visual Studio Build Tools depuis:
echo https://visualstudio.microsoft.com/fr/visual-cpp-build-tools/
pause
exit /b 1

:found
echo Configuration de l'environnement MSVC...
call "%VCVARS%" x64 >nul 2>&1

:: Verifier les fichiers
if not exist "main.c" (
    echo Erreur: main.c introuvable.
    pause
    exit /b 1
)

if not exist "resource.rc" (
    echo Erreur: resource.rc introuvable.
    pause
    exit /b 1
)

:: Etape 1: Compiler les ressources
echo [1/2] Compilation des ressources...
rc /nologo resource.rc
if %ERRORLEVEL% neq 0 (
    echo Erreur lors de la compilation des ressources.
    pause
    exit /b 1
)

:: Etape 2: Compiler le programme
echo [2/2] Compilation du programme...
cl /nologo /O2 /W3 /DNDEBUG /D_UNICODE /DUNICODE main.c resource.res /Fe:%EXE_NAME% /link /SUBSYSTEM:WINDOWS user32.lib gdi32.lib comctl32.lib shell32.lib kernel32.lib comdlg32.lib advapi32.lib

if %ERRORLEVEL% neq 0 (
    echo Erreur lors de la compilation.
    pause
    exit /b 1
)

:: Nettoyage
del /q *.obj *.res 2>nul

echo.
echo ============================================
echo Compilation terminee avec succes !
echo Executable: %EXE_NAME%
echo ============================================
pause

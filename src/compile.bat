@echo off
setlocal

:: Configuration
set CC=g++
set RES=windres
set EXE_NAME=la_meuh.exe
set MANIFEST=la_meuh.manifest
set RC_FILE=resource.rc

:: Vérifie que les outils sont disponibles
where %CC% >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo Erreur: g++ n'est pas installé.
    pause
    exit /b 1
)

where %RES% >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo Erreur: windres n'est pas installé.
    pause
    exit /b 1
)

:: Vérifie que les fichiers nécessaires existent
if not exist "%RC_FILE%" (
    echo Erreur: Fichier %RC_FILE% introuvable.
    pause
    exit /b 1
)

if not exist "main.cpp" (
    echo Erreur: Fichier main.cpp introuvable.
    pause
    exit /b 1
)

if not exist "%MANIFEST%" (
    echo Warning: Fichier %MANIFEST% introuvable. Le manifeste ne sera pas intégré.
    pause
)

:: Étape 1: Compilation des ressources (inclut le manifest)
echo [1/3] Compilation des ressources...
%RES% %RC_FILE% -o resource.o
if %ERRORLEVEL% neq 0 (
    echo Erreur lors de la compilation des ressources.
    pause
    exit /b 1
)

:: Vérification du fichier resource.o
echo Vérification du fichier resource.o :
dir resource.o
pause

:: Étape 2: Compilation du programme
echo [2/3] Compilation du programme...
%CC% -municode -mwindows main.cpp resource.o -o %EXE_NAME% -static -lcomctl32 -luser32 -lgdi32 -lshell32 -lcomdlg32 -lkernel32 -luxtheme -lole32 -luuid
if %ERRORLEVEL% neq 0 (
    echo Erreur lors de la compilation.
    pause
    exit /b 1
)

:: Vérification de l'exécutable
echo Vérification de l'exécutable :
dir %EXE_NAME%
pause

echo Compilation terminée avec succès !
pause

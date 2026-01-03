@echo off
setlocal

:: Configuration
set CC=gcc
set RES=windres
set EXE_NAME=la_meuh.exe
set MANIFEST=la_meuh.manifest
set RC_FILE=resource.rc

:: Vérifie que les outils sont disponibles
where %CC% >nul 2>&1
if %ERRORLEVEL% neq 0 (
    echo Erreur: gcc n'est pas installé.
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

if not exist "main.c" (
    echo Erreur: Fichier main.c introuvable.
    pause
    exit /b 1
)

:: Étape 1: Compilation des ressources
echo [1/3] Compilation des ressources...
%RES% %RC_FILE% -o resource.o
if %ERRORLEVEL% neq 0 (
    echo Erreur lors de la compilation des ressources.
    pause
    exit /b 1
)

:: Étape 2: Compilation du programme
echo [2/3] Compilation du programme...
%CC% main.c resource.o -o %EXE_NAME% -mwindows -static -lcomctl32 -luser32 -lgdi32 -lshell32 -lcomdlg32 -lkernel32 -luxtheme
if %ERRORLEVEL% neq 0 (
    echo Erreur lors de la compilation.
    pause
    exit /b 1
)

:: Étape 3: Intégration du manifeste
echo [3/3] Intégration du manifeste...
if exist "%MANIFEST%" (
    if exist "mt.exe" (
        mt.exe -manifest %MANIFEST% -outputresource:%EXE_NAME%;1
        if %ERRORLEVEL% neq 0 (
            echo Warning: Échec de l'intégration du manifeste. Vérifie que mt.exe est accessible.
            pause
        )
    ) else (
        echo Warning: mt.exe non trouvé. Le manifeste ne sera pas intégré.
        echo Pour l'intégrer manuellement, exécute:
        echo mt.exe -manifest %MANIFEST% -outputresource:%EXE_NAME%;1
    )
) else (
    echo Warning: Fichier %MANIFEST% introuvable.
)

echo Compilation terminée avec succès !
pause

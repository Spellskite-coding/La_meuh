#define _WIN32_IE 0x0300
#define _WIN32_WINNT 0x0501
#include <windows.h>
#include <tchar.h>
#include <commctrl.h>
#include <stdlib.h>
#include "resource.h"

#pragma execution_character_set("utf-8")

// Variables globales
HWND hProgressBar;
HWND hStatusLabel;
HWND hLogEdit;
HWND hUpdateButton;
HWND hQuitButton;
HBITMAP hMargueriteBitmap;
BOOL bUpdateInProgress = FALSE;
HANDLE hWingetProcess = NULL;  // Handle du processus winget pour fermeture propre

// Prototypes
LRESULT CALLBACK WindowProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam);
DWORD WINAPI UpdateThread(LPVOID lpParam);
void LogMessage(HWND hwnd, LPCSTR message);

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, LPSTR lpCmdLine, int nCmdShow)
{
    INITCOMMONCONTROLSEX icex = { sizeof(INITCOMMONCONTROLSEX) };
    icex.dwICC = ICC_PROGRESS_CLASS;
    InitCommonControlsEx(&icex);

    const char CLASS_NAME[] = "LaMeuhWindowClass";
    WNDCLASS wc = {0};
    wc.lpfnWndProc = WindowProc;
    wc.hInstance = hInstance;
    wc.lpszClassName = CLASS_NAME;
    wc.hIcon = LoadIcon(hInstance, MAKEINTRESOURCE(IDI_ICON1));
    wc.hCursor = LoadCursor(NULL, IDC_ARROW);
    RegisterClass(&wc);

    HWND hwnd = CreateWindowEx(
        0, CLASS_NAME, "La Meuh - Mises a jour",
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
        CW_USEDEFAULT, CW_USEDEFAULT, 450, 280,
        NULL, NULL, hInstance, NULL
    );
    if (hwnd == NULL) return 0;

    ShowWindow(hwnd, nCmdShow);
    UpdateWindow(hwnd);

    MSG msg;
    while (GetMessage(&msg, NULL, 0, 0))
    {
        TranslateMessage(&msg);
        DispatchMessage(&msg);
    }

    return 0;
}

LRESULT CALLBACK WindowProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam)
{
    switch (uMsg)
    {
        case WM_CREATE:
        {
            // Charge l'image depuis les ressources
            hMargueriteBitmap = (HBITMAP)LoadImage(
                GetModuleHandle(NULL),
                MAKEINTRESOURCE(IDB_MARGUERITE),
                IMAGE_BITMAP, 80, 80, LR_DEFAULTSIZE
            );

            // Image a gauche
            HWND hImage = CreateWindow(
                "STATIC", NULL, WS_VISIBLE | WS_CHILD | SS_BITMAP,
                15, 15, 80, 80, hwnd, (HMENU)100, NULL, NULL
            );
            if (hImage && hMargueriteBitmap)
            {
                SendMessage(hImage, STM_SETIMAGE, IMAGE_BITMAP, (LPARAM)hMargueriteBitmap);
            }

            // Status label a droite de l'image
            hStatusLabel = CreateWindow(
                "STATIC", "Pret a mettre a jour !",
                WS_VISIBLE | WS_CHILD | SS_LEFT,
                110, 30, 310, 40, hwnd, NULL, NULL, NULL
            );

            // Zone de log
            hLogEdit = CreateWindow(
                "EDIT", NULL,
                WS_VISIBLE | WS_CHILD | WS_VSCROLL | WS_BORDER | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY,
                15, 100, 405, 70, hwnd, (HMENU)2, NULL, NULL
            );

            // Barre de progression
            hProgressBar = CreateWindowEx(
                0, PROGRESS_CLASS, NULL,
                WS_VISIBLE | WS_CHILD | PBS_SMOOTH,
                15, 175, 405, 22, hwnd, NULL, NULL, NULL
            );
            SendMessage(hProgressBar, PBM_SETRANGE, 0, MAKELPARAM(0, 100));
            SendMessage(hProgressBar, PBM_SETSTEP, 1, 0);

            // Bouton Mettre a jour
            hUpdateButton = CreateWindow(
                "BUTTON", "Mettre a jour",
                WS_VISIBLE | WS_CHILD | BS_DEFPUSHBUTTON,
                15, 205, 200, 30, hwnd, (HMENU)1, NULL, NULL
            );

            // Bouton Quitter
            hQuitButton = CreateWindow(
                "BUTTON", "Quitter",
                WS_VISIBLE | WS_CHILD | BS_PUSHBUTTON,
                220, 205, 200, 30, hwnd, (HMENU)3, NULL, NULL
            );
            break;
        }
        case WM_COMMAND:
        {
            if (LOWORD(wParam) == 1) // Bouton Mettre a jour
            {
                if (!bUpdateInProgress)
                {
                    bUpdateInProgress = TRUE;
                    EnableWindow(hUpdateButton, FALSE);
                    SetWindowText(hUpdateButton, "En cours...");
                    SetWindowText(hStatusLabel, "Recherche de mises a jour...");
                    SendMessage(hLogEdit, WM_SETTEXT, 0, (LPARAM)"");
                    SendMessage(hProgressBar, PBM_SETPOS, 0, 0);
                    CreateThread(NULL, 0, UpdateThread, hwnd, 0, NULL);
                }
            }
            else if (LOWORD(wParam) == 3) // Bouton Quitter
            {
                if (bUpdateInProgress)
                {
                    if (MessageBox(hwnd, "Une mise a jour est en cours.\nVoulez-vous vraiment quitter ?",
                                   "Confirmation", MB_YESNO | MB_ICONQUESTION) == IDYES)
                    {
                        // Terminer winget avant de fermer
                        if (hWingetProcess != NULL)
                        {
                            TerminateProcess(hWingetProcess, 0);
                            CloseHandle(hWingetProcess);
                            hWingetProcess = NULL;
                        }
                        DestroyWindow(hwnd);
                    }
                }
                else
                {
                    DestroyWindow(hwnd);
                }
            }
            break;
        }
        case WM_DESTROY:
        {
            // Terminer winget proprement si en cours
            if (hWingetProcess != NULL)
            {
                TerminateProcess(hWingetProcess, 0);
                CloseHandle(hWingetProcess);
                hWingetProcess = NULL;
            }
            if (hMargueriteBitmap) DeleteObject(hMargueriteBitmap);
            PostQuitMessage(0);
            break;
        }
    }
    return DefWindowProc(hwnd, uMsg, wParam, lParam);
}

void LogMessage(HWND hwnd, LPCSTR message)
{
    // Convertir UTF-8 vers ANSI pour affichage correct
    // D'abord UTF-8 -> Unicode (Wide)
    int wlen = MultiByteToWideChar(CP_UTF8, 0, message, -1, NULL, 0);
    if (wlen > 0)
    {
        WCHAR* wbuffer = (WCHAR*)malloc(wlen * sizeof(WCHAR));
        if (wbuffer)
        {
            MultiByteToWideChar(CP_UTF8, 0, message, -1, wbuffer, wlen);

            // Puis Unicode -> ANSI (page de code locale)
            int alen = WideCharToMultiByte(CP_ACP, 0, wbuffer, -1, NULL, 0, NULL, NULL);
            if (alen > 0)
            {
                char* abuffer = (char*)malloc(alen);
                if (abuffer)
                {
                    WideCharToMultiByte(CP_ACP, 0, wbuffer, -1, abuffer, alen, NULL, NULL);
                    SendMessageA(hLogEdit, EM_REPLACESEL, 0, (LPARAM)abuffer);
                    SendMessageA(hLogEdit, EM_REPLACESEL, 0, (LPARAM)"\r\n");
                    free(abuffer);
                }
            }
            free(wbuffer);
        }
    }
}

DWORD WINAPI UpdateThread(LPVOID lpParam)
{
    HWND hwnd = (HWND)lpParam;
    SendMessage(hProgressBar, PBM_SETPOS, 0, 0);
    LogMessage(hwnd, "Verification de winget...");

    // Verifier si winget existe
    if (system("where winget >nul 2>&1") != 0)
    {
        SetWindowText(hStatusLabel, "Erreur: winget non trouve. Windows 11 requis.");
        return 1;
    }

    LogMessage(hwnd, "Lancement des mises a jour...");

    // Lancer winget en cache et capturer la sortie
    SECURITY_ATTRIBUTES sa = {sizeof(SECURITY_ATTRIBUTES), NULL, TRUE};
    HANDLE hRead, hWrite;
    if (!CreatePipe(&hRead, &hWrite, &sa, 0))
    {
        SetWindowText(hStatusLabel, "Erreur interne.");
        bUpdateInProgress = FALSE;
        EnableWindow(hUpdateButton, TRUE);
        SetWindowText(hUpdateButton, "Mettre a jour");
        return 1;
    }

    STARTUPINFOA si = {sizeof(si)};
    si.dwFlags = STARTF_USESHOWWINDOW | STARTF_USESTDHANDLES;
    si.wShowWindow = SW_HIDE;
    si.hStdOutput = hWrite;
    si.hStdError = hWrite;

    PROCESS_INFORMATION pi;
    char cmd[] = "winget upgrade --all --accept-package-agreements --accept-source-agreements --disable-interactivity";

    if (!CreateProcessA(NULL, cmd, NULL, NULL, TRUE, 0, NULL, NULL, &si, &pi))
    {
        CloseHandle(hRead);
        CloseHandle(hWrite);
        SetWindowText(hStatusLabel, "Erreur lors du lancement de winget.");
        bUpdateInProgress = FALSE;
        EnableWindow(hUpdateButton, TRUE);
        SetWindowText(hUpdateButton, "Mettre a jour");
        return 1;
    }

    // Stocker le handle pour fermeture propre
    hWingetProcess = pi.hProcess;
    CloseHandle(hWrite);

    SetWindowText(hStatusLabel, "Mise a jour en cours...");

    char buffer[4096];
    DWORD bytesRead;
    while (ReadFile(hRead, buffer, sizeof(buffer) - 1, &bytesRead, NULL) && bytesRead != 0)
    {
        buffer[bytesRead] = '\0';
        // Chercher des lignes interessantes
        if (strstr(buffer, "Trouv") || strstr(buffer, "Install") ||
            strstr(buffer, "Mise") || strstr(buffer, "Successfully") ||
            strstr(buffer, "Downloading") || strstr(buffer, "Starting") ||
            strstr(buffer, "Found"))
        {
            LogMessage(hwnd, buffer);
            SendMessage(hProgressBar, PBM_STEPIT, 0, 0);
        }
    }

    CloseHandle(hRead);
    WaitForSingleObject(pi.hProcess, INFINITE);

    DWORD exitCode;
    GetExitCodeProcess(pi.hProcess, &exitCode);
    CloseHandle(pi.hThread);
    CloseHandle(pi.hProcess);
    hWingetProcess = NULL;

    SendMessage(hProgressBar, PBM_SETPOS, 100, 0);

    if (exitCode == 0)
    {
        SetWindowText(hStatusLabel, "Tous les programmes sont a jour !");
        LogMessage(hwnd, "Mise a jour terminee avec succes !");
    }
    else
    {
        SetWindowText(hStatusLabel, "Mise a jour terminee.");
        LogMessage(hwnd, "Mise a jour terminee.");
    }

    // Reactiver le bouton
    bUpdateInProgress = FALSE;
    EnableWindow(hUpdateButton, TRUE);
    SetWindowText(hUpdateButton, "Mettre a jour");

    return 0;
}

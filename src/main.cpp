#define UNICODE
#define _UNICODE
#define _GNU_SOURCE
#define _WIN32_IE 0x0300
#define _WIN32_WINNT 0x0501

#include <windows.h>
#include <commctrl.h>
#include <stdlib.h>
#include <io.h>
#include <string>
#include <cstdio>
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
HANDLE hWingetProcess = NULL;
HANDLE g_hChildStd_OUT_Rd = NULL;
HANDLE g_hChildStd_OUT_Wr = NULL;

// Prototypes
LRESULT CALLBACK WindowProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam);
DWORD WINAPI UpdateThread(LPVOID lpParam);
void LogMessage(HWND hwnd, LPCSTR message);
BOOL ExecuteWingetUpgrade();
std::wstring FindWingetPath();

// Fonction pour trouver le chemin de winget.exe
std::wstring FindWingetPath()
{
    return L"winget";
}

BOOL ExecuteWingetUpgrade()
{
    SECURITY_ATTRIBUTES saAttr = { sizeof(SECURITY_ATTRIBUTES) };
    saAttr.bInheritHandle = TRUE;
    saAttr.lpSecurityDescriptor = NULL;

    // Crée un pipe pour la sortie standard
    if (!CreatePipe(&g_hChildStd_OUT_Rd, &g_hChildStd_OUT_Wr, &saAttr, 0))
        return FALSE;
    if (!SetHandleInformation(g_hChildStd_OUT_Rd, HANDLE_FLAG_INHERIT, 0))
        return FALSE;

    STARTUPINFOW si = { sizeof(si) };
    si.dwFlags = STARTF_USESHOWWINDOW | STARTF_USESTDHANDLES;
    si.wShowWindow = SW_HIDE;
    si.hStdOutput = g_hChildStd_OUT_Wr;
    si.hStdError = g_hChildStd_OUT_Wr;

    // Commande modifiée pour accepter automatiquement les contrats
    wchar_t cmdLine[MAX_PATH + 200];
    swprintf(cmdLine, MAX_PATH + 200, L"winget upgrade --all --silent --accept-source-agreements --accept-package-agreements");

    PROCESS_INFORMATION pi = {0};
    DWORD flags = CREATE_NO_WINDOW;
    BOOL result = CreateProcessW(
        NULL, cmdLine, NULL, NULL, TRUE, flags, NULL, NULL, &si, &pi
    );

    if (!result)
    {
        DWORD err = GetLastError();
        wchar_t errMsg[256];
        swprintf(errMsg, 256, L"Erreur CreateProcess: %d", err);
        MessageBoxW(NULL, errMsg, L"Debug", MB_OK);
        CloseHandle(g_hChildStd_OUT_Rd);
        CloseHandle(g_hChildStd_OUT_Wr);
        return FALSE;
    }

    CloseHandle(g_hChildStd_OUT_Wr);
    hWingetProcess = pi.hProcess;
    CloseHandle(pi.hThread);
    return TRUE;
}

int WINAPI wWinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, LPWSTR lpCmdLine, int nCmdShow)
{
    INITCOMMONCONTROLSEX icex = { sizeof(INITCOMMONCONTROLSEX) };
    icex.dwICC = ICC_PROGRESS_CLASS;
    InitCommonControlsEx(&icex);

    const wchar_t CLASS_NAME[] = L"LaMeuhWindowClass";
    WNDCLASSW wc = {0};
    wc.lpfnWndProc = WindowProc;
    wc.hInstance = hInstance;
    wc.lpszClassName = CLASS_NAME;
    wc.hIcon = LoadIconW(hInstance, MAKEINTRESOURCEW(IDI_ICON1));
    wc.hCursor = LoadCursorW(NULL, IDC_ARROW);
    wc.hbrBackground = (HBRUSH)(COLOR_BTNFACE + 1);
    RegisterClassW(&wc);

    HWND hwnd = CreateWindowExW(
        0, CLASS_NAME, L"La Meuh - Mises à jour",
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
        CW_USEDEFAULT, CW_USEDEFAULT, 450, 280,
        NULL, NULL, hInstance, NULL
    );
    if (hwnd == NULL) return 0;

    ShowWindow(hwnd, nCmdShow);
    UpdateWindow(hwnd);

    MSG msg;
    while (GetMessageW(&msg, NULL, 0, 0))
    {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    return 0;
}

LRESULT CALLBACK WindowProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam)
{
    switch (uMsg)
    {
        case WM_CREATE:
        {
            hMargueriteBitmap = (HBITMAP)LoadImageW(
                GetModuleHandleW(NULL),
                MAKEINTRESOURCEW(IDB_MARGUERITE),
                IMAGE_BITMAP, 80, 80, LR_DEFAULTSIZE
            );

            HWND hImage = CreateWindowW(
                L"STATIC", NULL, WS_VISIBLE | WS_CHILD | SS_BITMAP,
                15, 15, 80, 80, hwnd, (HMENU)100, NULL, NULL
            );
            if (hImage && hMargueriteBitmap)
            {
                SendMessageW(hImage, STM_SETIMAGE, IMAGE_BITMAP, (LPARAM)hMargueriteBitmap);
            }

            hStatusLabel = CreateWindowW(
                L"STATIC", L"Prêt à mettre à jour !",
                WS_VISIBLE | WS_CHILD | SS_LEFT,
                110, 30, 310, 40, hwnd, NULL, NULL, NULL
            );

            hLogEdit = CreateWindowW(
                L"EDIT", NULL,
                WS_VISIBLE | WS_CHILD | WS_VSCROLL | WS_BORDER | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY,
                15, 100, 405, 70, hwnd, (HMENU)2, NULL, NULL
            );

            hProgressBar = CreateWindowExW(
                0, PROGRESS_CLASSW, NULL,
                WS_VISIBLE | WS_CHILD | PBS_SMOOTH,
                15, 175, 405, 22, hwnd, NULL, NULL, NULL
            );
            SendMessageW(hProgressBar, PBM_SETRANGE, 0, MAKELPARAM(0, 100));
            SendMessageW(hProgressBar, PBM_SETSTEP, 1, 0);

            hUpdateButton = CreateWindowW(
                L"BUTTON", L"Mettre à jour",
                WS_VISIBLE | WS_CHILD | BS_DEFPUSHBUTTON,
                15, 205, 200, 30, hwnd, (HMENU)1, NULL, NULL
            );

            hQuitButton = CreateWindowW(
                L"BUTTON", L"Quitter",
                WS_VISIBLE | WS_CHILD | BS_PUSHBUTTON,
                220, 205, 200, 30, hwnd, (HMENU)3, NULL, NULL
            );
            break;
        }
        case WM_COMMAND:
        {
            if (LOWORD(wParam) == 1) // Bouton Mettre à jour
            {
                if (!bUpdateInProgress)
                {
                    bUpdateInProgress = TRUE;
                    EnableWindow(hUpdateButton, FALSE);
                    SetWindowTextW(hUpdateButton, L"En cours...");
                    SetWindowTextW(hStatusLabel, L"Recherche de mises à jour...");
                    SendMessageW(hLogEdit, WM_SETTEXT, 0, (LPARAM)L"");
                    SendMessageW(hProgressBar, PBM_SETPOS, 0, 0);
                    CreateThread(NULL, 0, UpdateThread, hwnd, 0, NULL);
                }
            }
            else if (LOWORD(wParam) == 3) // Bouton Quitter
            {
                if (bUpdateInProgress)
                {
                    if (MessageBoxW(hwnd, L"Une mise à jour est en cours.\nVoulez-vous vraiment quitter ?",
                                   L"Confirmation", MB_YESNO | MB_ICONQUESTION) == IDYES)
                    {
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
    return DefWindowProcW(hwnd, uMsg, wParam, lParam);
}

void LogMessage(HWND hwnd, LPCSTR message)
{
    int wlen = MultiByteToWideChar(CP_UTF8, 0, message, -1, NULL, 0);
    if (wlen > 0)
    {
        WCHAR* wbuffer = new WCHAR[wlen];
        MultiByteToWideChar(CP_UTF8, 0, message, -1, wbuffer, wlen);
        SendMessageW(hLogEdit, EM_REPLACESEL, 0, (LPARAM)wbuffer);
        SendMessageW(hLogEdit, EM_REPLACESEL, 0, (LPARAM)L"\r\n");
        delete[] wbuffer;
    }
}

DWORD WINAPI UpdateThread(LPVOID lpParam)
{
    HWND hwnd = (HWND)lpParam;
    SendMessageW(hProgressBar, PBM_SETPOS, 0, 0);
    LogMessage(hwnd, "Recherche de winget...");

    LogMessage(hwnd, "Lancement des mises à jour...");
    if (!ExecuteWingetUpgrade())
    {
        LogMessage(hwnd, "Erreur: Impossible de lancer winget.");
        SetWindowTextW(hStatusLabel, L"Erreur lors du lancement de winget.");
        bUpdateInProgress = FALSE;
        EnableWindow(hUpdateButton, TRUE);
        SetWindowTextW(hUpdateButton, L"Mettre à jour");
        return 1;
    }

    SetWindowTextW(hStatusLabel, L"Mise à jour en cours...");
    SendMessageW(hProgressBar, PBM_SETPOS, 50, 0);

    // Buffer pour lire la sortie de winget
    CHAR buffer[4096];
    DWORD bytesRead;
    std::string output;
    BOOL foundUpdates = FALSE;

    // Lire la sortie de winget en temps réel
    while (ReadFile(g_hChildStd_OUT_Rd, buffer, sizeof(buffer) - 1, &bytesRead, NULL) && bytesRead != 0)
    {
        buffer[bytesRead] = '\0';
        output += buffer;
        // On cherche "Trouvé X mise(s) à jour" ou "Aucune mise à jour disponible"
        if (strstr(buffer, "Trouvé ") && strstr(buffer, " mise(s) à jour"))
            foundUpdates = TRUE;
        if (strstr(buffer, "Aucune mise à jour disponible"))
            foundUpdates = FALSE;
    }

    // Attendre la fin du processus avec un timeout
    DWORD waitResult = WaitForSingleObject(hWingetProcess, 30000);
    if (waitResult == WAIT_TIMEOUT)
    {
        TerminateProcess(hWingetProcess, 0);
        LogMessage(hwnd, "Winget n'a pas répondu dans le temps imparti.");
        SetWindowTextW(hStatusLabel, L"Aucune mise à jour nécessaire.");
    }
    else
    {
        DWORD exitCode;
        GetExitCodeProcess(hWingetProcess, &exitCode);
        if (foundUpdates)
        {
            LogMessage(hwnd, "Mise à jour terminée.");
            SetWindowTextW(hStatusLabel, L"Mise à jour terminée !");
        }
        else
        {
            LogMessage(hwnd, "Aucune mise à jour nécessaire.");
            SetWindowTextW(hStatusLabel, L"Aucune mise à jour nécessaire.");
        }
    }

    CloseHandle(g_hChildStd_OUT_Rd);
    CloseHandle(hWingetProcess);
    hWingetProcess = NULL;
    SendMessageW(hProgressBar, PBM_SETPOS, 100, 0);

    bUpdateInProgress = FALSE;
    EnableWindow(hUpdateButton, TRUE);
    SetWindowTextW(hUpdateButton, L"Mettre à jour");

    return 0;
}

#define _UNICODE
#define _GNU_SOURCE
#define _WIN32_IE 0x0300
#define _WIN32_WINNT 0x0501

#include <windows.h>
#include <commctrl.h>
#include <cwchar>  // Pour swprintf
#include "resource.h"

#pragma execution_character_set("utf-8")

// Messages personnalisés pour la communication entre threads
#define WM_UPDATE_LOG (WM_USER + 1)
#define WM_UPDATE_STATUS (WM_USER + 2)
#define WM_UPDATE_BUTTON (WM_USER + 3)

// Variables globales
HWND hStatusLabel;
HWND hLogEdit;
HWND hUpdateButton;
HWND hQuitButton;
HBITMAP hMargueriteBitmap;
BOOL bUpdateInProgress = FALSE;
HANDLE hWingetProcess = NULL;

// Prototypes
LRESULT CALLBACK WindowProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam);
DWORD WINAPI UpdateThread(LPVOID lpParam);
BOOL ExecuteWingetUpgrade();

BOOL ExecuteWingetUpgrade()
{
    STARTUPINFOW si = { sizeof(si) };
    si.dwFlags = STARTF_USESHOWWINDOW;
    si.wShowWindow = SW_HIDE;

    wchar_t cmdLine[MAX_PATH + 200];
    swprintf(cmdLine, MAX_PATH + 200, L"winget upgrade --all --silent --accept-source-agreements --accept-package-agreements");

    PROCESS_INFORMATION pi = {0};
    BOOL result = CreateProcessW(NULL, cmdLine, NULL, NULL, FALSE, CREATE_NO_WINDOW, NULL, NULL, &si, &pi);

    if (!result)
    {
        return FALSE;
    }

    hWingetProcess = pi.hProcess;
    CloseHandle(pi.hThread);
    return TRUE;
}

int WINAPI wWinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, LPWSTR lpCmdLine, int nCmdShow)
{
    INITCOMMONCONTROLSEX icex = { sizeof(INITCOMMONCONTROLSEX) };
    icex.dwICC = ICC_STANDARD_CLASSES;
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

    // Création de la fenêtre
    HWND hwnd = CreateWindowExW(
        0, CLASS_NAME, L"La Meuh - Mises à jour",
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
        CW_USEDEFAULT, CW_USEDEFAULT, 450, 320,
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

            // Image de la vache
            HWND hImage = CreateWindowW(
                L"STATIC", NULL, WS_VISIBLE | WS_CHILD | SS_BITMAP,
                15, 10, 80, 80, hwnd, (HMENU)100, NULL, NULL
            );
            if (hImage && hMargueriteBitmap)
            {
                SendMessageW(hImage, STM_SETIMAGE, IMAGE_BITMAP, (LPARAM)hMargueriteBitmap);
            }

            // Label de statut
            hStatusLabel = CreateWindowW(
                L"STATIC", L"Prêt à mettre à jour !",
                WS_VISIBLE | WS_CHILD | SS_LEFT,
                110, 20, 300, 30, hwnd, NULL, NULL, NULL
            );

            // Zone de log
            hLogEdit = CreateWindowW(
                L"EDIT", NULL,
                WS_VISIBLE | WS_CHILD | WS_VSCROLL | WS_BORDER | ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY,
                15, 100, 405, 130,
                hwnd, (HMENU)2, NULL, NULL
            );

            // Bouton "Mettre à jour"
            hUpdateButton = CreateWindowW(
                L"BUTTON", L"Mettre à jour",
                WS_VISIBLE | WS_CHILD | BS_DEFPUSHBUTTON,
                70, 250, 150, 30,
                hwnd, (HMENU)1, NULL, NULL
            );

            // Bouton "Quitter"
            hQuitButton = CreateWindowW(
                L"BUTTON", L"Quitter",
                WS_VISIBLE | WS_CHILD | BS_PUSHBUTTON,
                230, 250, 150, 30,
                hwnd, (HMENU)3, NULL, NULL
            );
            break;
        }
        case WM_COMMAND:
        {
            if (LOWORD(wParam) == 1 && !bUpdateInProgress)
            {
                bUpdateInProgress = TRUE;
                EnableWindow(hUpdateButton, FALSE);
                SetWindowTextW(hUpdateButton, L"En cours...");
                SetWindowTextW(hStatusLabel, L"Recherche de mises à jour...");
                SendMessageW(hLogEdit, WM_SETTEXT, 0, (LPARAM)L"");
                CreateThread(NULL, 0, UpdateThread, hwnd, 0, NULL);
            }
            else if (LOWORD(wParam) == 3)
            {
                if (bUpdateInProgress)
                {
                    if (MessageBoxW(hwnd, L"Une mise à jour est en cours.\nVoulez-vous vraiment quitter ?",
                                   L"Confirmation", MB_YESNO | MB_ICONQUESTION) == IDYES)
                    {
                        if (hWingetProcess)
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
        case WM_UPDATE_LOG:
        {
            const char* message = (const char*)lParam;
            int wlen = MultiByteToWideChar(CP_UTF8, 0, message, -1, NULL, 0);
            if (wlen > 0)
            {
                WCHAR* wbuffer = new WCHAR[wlen];
                MultiByteToWideChar(CP_UTF8, 0, message, -1, wbuffer, wlen);
                int length = GetWindowTextLengthW(hLogEdit);
                SendMessageW(hLogEdit, EM_SETSEL, length, length);
                SendMessageW(hLogEdit, EM_REPLACESEL, 0, (LPARAM)wbuffer);
                SendMessageW(hLogEdit, EM_REPLACESEL, 0, (LPARAM)L"\r\n");
                delete[] wbuffer;
            }
            break;
        }
        case WM_UPDATE_STATUS:
        {
            SetWindowTextW(hStatusLabel, (LPCWSTR)lParam);
            break;
        }
        case WM_UPDATE_BUTTON:
        {
            EnableWindow(hUpdateButton, TRUE);
            SetWindowTextW(hUpdateButton, (LPCWSTR)lParam);
            break;
        }
        case WM_DESTROY:
        {
            if (hWingetProcess)
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

DWORD WINAPI UpdateThread(LPVOID lpParam)
{
    HWND hwnd = (HWND)lpParam;
    PostMessageW(hwnd, WM_UPDATE_LOG, 0, (LPARAM)"Recherche de mises à jour en cours...");

    if (!ExecuteWingetUpgrade())
    {
        PostMessageW(hwnd, WM_UPDATE_LOG, 0, (LPARAM)"Erreur: Impossible de lancer winget.");
        PostMessageW(hwnd, WM_UPDATE_STATUS, 0, (LPARAM)L"Erreur lors du lancement de winget.");
        PostMessageW(hwnd, WM_UPDATE_BUTTON, 0, (LPARAM)L"Mettre à jour");
        bUpdateInProgress = FALSE;
        return 1;
    }

    PostMessageW(hwnd, WM_UPDATE_STATUS, 0, (LPARAM)L"Mise à jour en cours...");

    // Attendre la fin du processus winget
    WaitForSingleObject(hWingetProcess, INFINITE);
    DWORD exitCode;
    GetExitCodeProcess(hWingetProcess, &exitCode);
    CloseHandle(hWingetProcess);
    hWingetProcess = NULL;

    // Afficher le résultat
    PostMessageW(hwnd, WM_UPDATE_LOG, 0, (LPARAM)"Vérification des mises à jour terminée.");
    PostMessageW(hwnd, WM_UPDATE_STATUS, 0, (LPARAM)L"Aucune mise à jour nécessaire.");
    PostMessageW(hwnd, WM_UPDATE_BUTTON, 0, (LPARAM)L"Mettre à jour");
    bUpdateInProgress = FALSE;

    return 0;
}

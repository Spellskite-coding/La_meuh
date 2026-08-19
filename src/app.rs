//! État de la fenêtre principale et procédure de fenêtre (WndProc).
//!
//! Contrairement au C++ d'origine, qui gardait `bUpdateInProgress` et
//! `hWingetProcess` dans des variables globales lues/écrites sans aucune
//! synchronisation depuis le thread UI *et* depuis le thread de mise à jour
//! (une vraie course: le thread UI pouvait lire un HANDLE que le thread de
//! fond était en train de fermer, puis appeler `TerminateProcess`/
//! `CloseHandle` sur un HANDLE potentiellement déjà recyclé par le système
//! pour un tout autre objet noyau), l'état partagé est ici protégé par un
//! `Mutex` et un `AtomicBool`.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::Duration;

use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{DeleteObject, HBITMAP};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::SystemServices::{SS_BITMAP, SS_LEFT};
use windows::Win32::UI::Controls::{EM_REPLACESEL, EM_SETSEL};
use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::process::{spawn_winget, RunningWinget, WorkerMsg};
use crate::resources::*;
use crate::winget::locate_winget;

const CANCEL_GRACE_PERIOD: Duration = Duration::from_secs(5);

pub struct AppState {
    status_label: Cell<HWND>,
    log_edit: Cell<HWND>,
    update_button: Cell<HWND>,
    quit_button: Cell<HWND>,
    marguerite_bitmap: Cell<HBITMAP>,
    update_in_progress: AtomicBool,
    running: Mutex<Option<RunningWinget>>,
    log_rx: Mutex<Option<Receiver<WorkerMsg>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            status_label: Cell::new(HWND::default()),
            log_edit: Cell::new(HWND::default()),
            update_button: Cell::new(HWND::default()),
            quit_button: Cell::new(HWND::default()),
            marguerite_bitmap: Cell::new(HBITMAP::default()),
            update_in_progress: AtomicBool::new(false),
            running: Mutex::new(None),
            log_rx: Mutex::new(None),
        }
    }
}

pub unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = lparam.0 as *const CREATESTRUCTW;
        if !cs.is_null() {
            let state_ptr = (*cs).lpCreateParams as isize;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr);
        }
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AppState;
    if state_ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let state = &*state_ptr;

    match msg {
        WM_CREATE => {
            on_create(hwnd, state);
            LRESULT(0)
        }
        WM_COMMAND => {
            on_command(hwnd, state, wparam);
            LRESULT(0)
        }
        WM_APP_WORKER_EVENT => {
            on_worker_event(state);
            LRESULT(0)
        }
        WM_DESTROY => {
            on_destroy(state);
            PostQuitMessage(0);
            LRESULT(0)
        }
        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Box::from_raw(state_ptr as *mut AppState));
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn on_create(hwnd: HWND, state: &AppState) {
    let bitmap = LoadImageW(
        GetModuleHandleW(None).unwrap_or_default(),
        resource_id(IDB_MARGUERITE),
        IMAGE_BITMAP,
        80,
        80,
        LR_DEFAULTSIZE,
    )
    .map(|h| HBITMAP(h.0))
    .unwrap_or_default();
    state.marguerite_bitmap.set(bitmap);

    let image = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        w!(""),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(SS_BITMAP.0),
        15,
        10,
        80,
        80,
        hwnd,
        HMENU(ID_BITMAP as *mut _),
        None,
        None,
    )
    .ok();
    if let Some(image) = image {
        if !bitmap.is_invalid() {
            SendMessageW(
                image,
                STM_SETIMAGE,
                WPARAM(IMAGE_BITMAP.0 as usize),
                LPARAM(bitmap.0 as isize),
            );
        }
    }

    let status = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        w!("Prêt à mettre à jour !"),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(SS_LEFT.0),
        110,
        20,
        300,
        30,
        hwnd,
        None,
        None,
        None,
    )
    .unwrap_or_default();
    state.status_label.set(status);

    let log_edit = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("EDIT"),
        w!(""),
        WS_VISIBLE
            | WS_CHILD
            | WS_VSCROLL
            | WS_BORDER
            | WINDOW_STYLE(ES_MULTILINE as u32)
            | WINDOW_STYLE(ES_AUTOVSCROLL as u32)
            | WINDOW_STYLE(ES_READONLY as u32),
        15,
        100,
        405,
        130,
        hwnd,
        HMENU(ID_LOG_EDIT as *mut _),
        None,
        None,
    )
    .unwrap_or_default();
    state.log_edit.set(log_edit);

    let update_button = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("Mettre à jour"),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
        70,
        250,
        150,
        30,
        hwnd,
        HMENU(ID_UPDATE_BUTTON as *mut _),
        None,
        None,
    )
    .unwrap_or_default();
    state.update_button.set(update_button);

    let quit_button = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        w!("Quitter"),
        WS_VISIBLE | WS_CHILD | WINDOW_STYLE(BS_PUSHBUTTON as u32),
        230,
        250,
        150,
        30,
        hwnd,
        HMENU(ID_QUIT_BUTTON as *mut _),
        None,
        None,
    )
    .unwrap_or_default();
    state.quit_button.set(quit_button);
}

unsafe fn on_command(hwnd: HWND, state: &AppState, wparam: WPARAM) {
    let id = (wparam.0 & 0xFFFF) as i32;

    if id == ID_UPDATE_BUTTON {
        if state.update_in_progress.swap(true, Ordering::SeqCst) {
            return; // déjà en cours: le C++ d'origine ignorait aussi les double-clics.
        }
        start_update(hwnd, state);
    } else if id == ID_QUIT_BUTTON {
        if state.update_in_progress.load(Ordering::SeqCst) {
            let answer = MessageBoxW(
                hwnd,
                w!("Une mise à jour est en cours.\nVoulez-vous vraiment quitter ?\n\nLa fermeture demandera à winget de s'arrêter proprement."),
                w!("Confirmation"),
                MB_YESNO | MB_ICONQUESTION,
            );
            if answer == IDYES {
                let _ = SetWindowTextW(state.status_label.get(), w!("Annulation en cours..."));
                let _ = EnableWindow(state.quit_button.get(), false);
                // Ne pas bloquer le thread UI: l'arrêt propre (CTRL_BREAK
                // puis, seulement si besoin, TerminateProcess après un
                // délai de grâce) se fait sur un thread dédié.
                if let Some(running) = state.running.lock().unwrap().take() {
                    let hwnd_send = crate::resources::SendHwnd::new(hwnd);
                    std::thread::spawn(move || {
                        running.request_stop(CANCEL_GRACE_PERIOD);
                        let _ = PostMessageW(hwnd_send.get(), WM_CLOSE, WPARAM(0), LPARAM(0));
                    });
                } else {
                    let _ = DestroyWindow(hwnd);
                }
            }
        } else {
            let _ = DestroyWindow(hwnd);
        }
    }
}

unsafe fn start_update(hwnd: HWND, state: &AppState) {
    let _ = EnableWindow(state.update_button.get(), false);
    let _ = SetWindowTextW(state.update_button.get(), w!("En cours..."));
    let _ = SetWindowTextW(state.status_label.get(), w!("Recherche de mises à jour..."));
    let _ = SetWindowTextW(state.log_edit.get(), w!(""));

    let (tx, rx) = channel::<WorkerMsg>();
    *state.log_rx.lock().unwrap() = Some(rx);

    match locate_winget() {
        None => {
            let _ = tx.send(WorkerMsg::Log(
                "Erreur: winget est introuvable sur ce système.\r\n\
                 Installez \"App Installer\" depuis le Microsoft Store, puis réessayez.\r\n"
                    .to_string(),
            ));
            let _ = tx.send(WorkerMsg::Status("winget est introuvable.".to_string()));
            let _ = tx.send(WorkerMsg::Finished);
            let _ = PostMessageW(hwnd, WM_APP_WORKER_EVENT, WPARAM(0), LPARAM(0));
        }
        Some(path) => {
            let _ = tx.send(WorkerMsg::Log(format!(
                "Lancement de winget ({})...\r\n",
                path.display()
            )));
            let _ = PostMessageW(hwnd, WM_APP_WORKER_EVENT, WPARAM(0), LPARAM(0));

            // On garde un clone du sender: `spawn_winget` consomme `tx` (et
            // le laisse retomber si le lancement échoue avant même de créer
            // le thread lecteur), mais le récepteur `rx` reste stocké dans
            // `state.log_rx` avec le message ci-dessus déjà dedans — il ne
            // faut donc pas le remplacer par un nouveau canal en cas
            // d'erreur, sous peine de perdre ce message silencieusement.
            let tx_for_errors = tx.clone();
            match spawn_winget(&path, hwnd, tx) {
                Ok(running) => {
                    *state.running.lock().unwrap() = Some(running);
                }
                Err(e) => {
                    let _ = tx_for_errors.send(WorkerMsg::Log(format!("Erreur: {e}\r\n")));
                    let _ = tx_for_errors.send(WorkerMsg::Status(
                        "Erreur lors du lancement de winget.".to_string(),
                    ));
                    let _ = tx_for_errors.send(WorkerMsg::Finished);
                    let _ = PostMessageW(hwnd, WM_APP_WORKER_EVENT, WPARAM(0), LPARAM(0));
                }
            }
        }
    }
}

unsafe fn on_worker_event(state: &AppState) {
    let guard = state.log_rx.lock().unwrap();
    let Some(rx) = guard.as_ref() else { return };

    while let Ok(msg) = rx.try_recv() {
        match msg {
            WorkerMsg::Log(text) => append_log(state, &text),
            WorkerMsg::Status(text) => {
                let wide = to_wide(&text);
                let _ = SetWindowTextW(
                    state.status_label.get(),
                    windows::core::PCWSTR(wide.as_ptr()),
                );
            }
            WorkerMsg::Finished => {
                state.update_in_progress.store(false, Ordering::SeqCst);
                *state.running.lock().unwrap() = None;
                let _ = EnableWindow(state.update_button.get(), true);
                let _ = SetWindowTextW(state.update_button.get(), w!("Mettre à jour"));
            }
        }
    }
}

unsafe fn append_log(state: &AppState, text: &str) {
    let wide = to_wide(text);
    let log_edit = state.log_edit.get();
    let length = GetWindowTextLengthW(log_edit);
    SendMessageW(
        log_edit,
        EM_SETSEL,
        WPARAM(length as usize),
        LPARAM(length as isize),
    );
    SendMessageW(
        log_edit,
        EM_REPLACESEL,
        WPARAM(0),
        LPARAM(wide.as_ptr() as isize),
    );
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn on_destroy(state: &AppState) {
    if let Some(running) = state.running.lock().unwrap().take() {
        running.request_stop(Duration::from_millis(500));
    }
    let bitmap = state.marguerite_bitmap.get();
    if !bitmap.is_invalid() {
        let _ = DeleteObject(bitmap);
    }
}

unsafe fn resource_id(id: u16) -> windows::core::PCWSTR {
    windows::core::PCWSTR(id as usize as *const u16)
}

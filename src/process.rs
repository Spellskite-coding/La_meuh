//! Lancement de winget et lecture asynchrone de sa sortie, avec annulation
//! "propre" (au lieu d'un `TerminateProcess` immédiat et brutal comme dans
//! la version C++ d'origine, qui pouvait couper un gestionnaire de paquets
//! en pleine installation/désinstallation et laisser un paquet dans un état
//! incohérent).

use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{
    CloseHandle, SetHandleInformation, HANDLE, HANDLE_FLAGS, HANDLE_FLAG_INHERIT, HWND,
    WAIT_TIMEOUT,
};
use windows::Win32::Storage::FileSystem::ReadFile;
use windows::Win32::System::Console::{GenerateConsoleCtrlEvent, CTRL_BREAK_EVENT};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, TerminateProcess, WaitForSingleObject,
    CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION,
    STARTF_USESHOWWINDOW, STARTF_USESTDHANDLES, STARTUPINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SW_HIDE};

use crate::resources::WM_APP_WORKER_EVENT;
use crate::winget::{build_command_line, to_wide_path};

/// Enveloppe RAII autour d'un HANDLE Win32: garantit `CloseHandle` sur tous
/// les chemins de sortie, y compris les retours d'erreur anticipés. Le code
/// C++ d'origine avait une fuite de handles sur l'un de ces chemins (un
/// retour anticipé après un `SetHandleInformation` en échec, avant la
/// fermeture des deux bouts du pipe).
struct HandleGuard(HANDLE);

// SAFETY: un HANDLE Win32 est une valeur opaque (jamais déréférencée comme
// pointeur par ce code); le déplacer vers un autre thread pour y appeler
// CloseHandle/WaitForSingleObject/ReadFile est le usage normal et documenté
// de ces API.
unsafe impl Send for HandleGuard {}

impl Drop for HandleGuard {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Messages envoyés du thread de fond vers le thread UI. Le message Windows
/// `WM_APP_WORKER_EVENT` ne fait que signaler "de nouveaux messages sont
/// disponibles"; toutes les données transitent par ce canal, jamais par un
/// pointeur brut passé à `PostMessage` (cf. resources.rs).
pub enum WorkerMsg {
    Log(String),
    Status(String),
    Finished,
}

/// Un winget en cours d'exécution. Détient le HANDLE process pour pouvoir
/// l'annuler proprement.
pub struct RunningWinget {
    process: HANDLE,
    pid: u32,
}

// SAFETY: même raisonnement que pour `HandleGuard`: `process` est une valeur
// de HANDLE opaque, jamais déréférencée comme pointeur.
unsafe impl Send for RunningWinget {}

impl RunningWinget {
    /// Demande un arrêt "propre": envoie CTRL_BREAK au groupe de process de
    /// winget (créé avec CREATE_NEW_PROCESS_GROUP, donc indépendant de la
    /// console de La Meuh) puis attend `timeout`. Si winget ne s'est pas
    /// arrêté de lui-même, on le termine en dernier recours seulement.
    pub fn request_stop(&self, timeout: Duration) {
        unsafe {
            let _ = GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, self.pid);
            let waited = WaitForSingleObject(self.process, timeout.as_millis() as u32);
            if waited == WAIT_TIMEOUT {
                let _ = TerminateProcess(self.process, 1);
            }
        }
    }
}

impl Drop for RunningWinget {
    fn drop(&mut self) {
        if !self.process.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.process);
            }
        }
    }
}

/// Lance `winget upgrade --all ...` sans fenêtre console visible, dans son
/// propre groupe de processus (nécessaire pour pouvoir lui envoyer
/// CTRL_BREAK indépendamment de La Meuh), et démarre un thread qui relaie sa
/// sortie standard vers `sender`, en réveillant la fenêtre `hwnd` à chaque
/// lot de messages.
pub fn spawn_winget(
    winget_path: &Path,
    hwnd: HWND,
    sender: Sender<WorkerMsg>,
) -> Result<RunningWinget, String> {
    let security_attributes = windows::Win32::Security::SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<windows::Win32::Security::SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: windows::Win32::Foundation::TRUE,
    };

    let (read_handle, write_handle) = unsafe {
        let mut read = HANDLE::default();
        let mut write = HANDLE::default();
        CreatePipe(
            &mut read,
            &mut write,
            Some(&security_attributes as *const windows::Win32::Security::SECURITY_ATTRIBUTES),
            0,
        )
        .map_err(|e| format!("CreatePipe a échoué: {e}"))?;
        (HandleGuard(read), HandleGuard(write))
    };

    unsafe {
        SetHandleInformation(read_handle.0, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))
            .map_err(|e| format!("SetHandleInformation a échoué: {e}"))?;
    }

    let startup_info = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        dwFlags: STARTF_USESHOWWINDOW | STARTF_USESTDHANDLES,
        wShowWindow: SW_HIDE.0 as u16,
        hStdOutput: write_handle.0,
        hStdError: write_handle.0,
        ..Default::default()
    };

    let app_name = to_wide_path(winget_path);
    let mut cmdline = build_command_line(winget_path);

    let mut process_info = PROCESS_INFORMATION::default();
    let flags = PROCESS_CREATION_FLAGS(CREATE_NO_WINDOW.0 | CREATE_NEW_PROCESS_GROUP.0);

    let spawn_result = unsafe {
        CreateProcessW(
            PCWSTR(app_name.as_ptr()),
            windows::core::PWSTR(cmdline.as_mut_ptr()),
            None,
            None,
            true,
            flags,
            None,
            None,
            &startup_info,
            &mut process_info,
        )
    };

    if let Err(e) = spawn_result {
        return Err(format!("Impossible de lancer winget: {e}"));
    }

    // Le bout écriture du pipe ne doit vivre que dans le process enfant.
    drop(write_handle);
    unsafe {
        let _ = CloseHandle(process_info.hThread);
    }

    // Le HANDLE de process est utilisé par DEUX consommateurs indépendants
    // et concurrents: le thread de lecture ci-dessous (WaitForSingleObject +
    // GetExitCodeProcess en fin de lecture) et `RunningWinget`, gardé côté
    // UI pour permettre une annulation depuis le bouton Quitter. Leur donner
    // à chacun sa PROPRE copie dupliquée (au lieu de partager la même valeur
    // de handle comme le ferait un simple Copy) évite un use-after-close:
    // sans ça, si l'un des deux ferme "le" handle pendant que l'autre est
    // encore en train de l'utiliser (ou vient d'être réveillé par
    // WaitForSingleObject et s'apprête à appeler GetExitCodeProcess), le
    // système peut avoir déjà recyclé ce numéro de handle pour un tout autre
    // objet noyau — comportement non défini au sens Win32.
    let process_for_thread = unsafe { duplicate_handle(process_info.hProcess) }
        .map_err(|e| format!("DuplicateHandle a échoué: {e}"))?;

    let running = RunningWinget {
        process: process_info.hProcess,
        pid: process_info.dwProcessId,
    };

    let hwnd_send = crate::resources::SendHwnd::new(hwnd);
    let process_guard = HandleGuard(process_for_thread);
    std::thread::spawn(move || {
        read_output_and_wait(read_handle, process_guard, hwnd_send.get(), sender);
    });

    Ok(running)
}

/// Duplique un HANDLE au sein du process courant, avec les mêmes droits
/// d'accès, pour donner à deux propriétaires indépendants chacun leur propre
/// handle qu'ils peuvent fermer sans affecter l'autre.
unsafe fn duplicate_handle(source: HANDLE) -> windows::core::Result<HANDLE> {
    use windows::Win32::Foundation::{DuplicateHandle, DUPLICATE_SAME_ACCESS};
    use windows::Win32::System::Threading::GetCurrentProcess;
    let current = GetCurrentProcess();
    let mut target = HANDLE::default();
    DuplicateHandle(
        current,
        source,
        current,
        &mut target,
        0,
        false,
        DUPLICATE_SAME_ACCESS,
    )?;
    Ok(target)
}

fn read_output_and_wait(
    read_handle: HandleGuard,
    process: HandleGuard,
    hwnd: HWND,
    sender: Sender<WorkerMsg>,
) {
    let _ = sender.send(WorkerMsg::Status("Mise à jour en cours...".to_string()));
    wake_ui(hwnd);

    let mut raw_buf = [0u8; 4096];
    // Bytes UTF-8 incomplets laissés par la lecture précédente, à
    // recoller devant la prochaine lecture: `ReadFile` peut couper une
    // séquence multi-octets pile à la frontière du buffer, ce que le code
    // C++ d'origine ne gérait pas (il pouvait produire des caractères
    // corrompus dans le log en cas de coupure malchanceuse).
    let mut pending: Vec<u8> = Vec::new();

    loop {
        let mut bytes_read = 0u32;
        let ok = unsafe {
            ReadFile(
                read_handle.0,
                Some(&mut raw_buf[..]),
                Some(&mut bytes_read as *mut u32),
                None,
            )
        };
        if ok.is_err() || bytes_read == 0 {
            break;
        }

        pending.extend_from_slice(&raw_buf[..bytes_read as usize]);
        let valid_len = match std::str::from_utf8(&pending) {
            Ok(_) => pending.len(),
            Err(e) => e.valid_up_to(),
        };
        if valid_len > 0 {
            let text = String::from_utf8_lossy(&pending[..valid_len]).into_owned();
            let _ = sender.send(WorkerMsg::Log(text));
            pending.drain(..valid_len);
            wake_ui(hwnd);
        }
    }

    if !pending.is_empty() {
        let text = String::from_utf8_lossy(&pending).into_owned();
        let _ = sender.send(WorkerMsg::Log(text));
    }

    let mut exit_code = 0u32;
    unsafe {
        let _ = WaitForSingleObject(process.0, u32::MAX);
        let _ = GetExitCodeProcess(process.0, &mut exit_code);
    }

    let status = if exit_code == 0 {
        "Mise à jour terminée.".to_string()
    } else {
        format!("winget s'est terminé avec le code {exit_code}.")
    };
    let _ = sender.send(WorkerMsg::Status(status));
    let _ = sender.send(WorkerMsg::Finished);
    wake_ui(hwnd);
}

fn wake_ui(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(
            hwnd,
            WM_APP_WORKER_EVENT,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(0),
        );
    }
}

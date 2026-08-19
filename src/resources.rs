//! Identifiants partagés avec `resources/la_meuh.rc`.

pub const IDI_ICON1: u16 = 101;
pub const IDB_MARGUERITE: u16 = 102;

/// Identifiants des contrôles enfants de la fenêtre principale.
pub const ID_BITMAP: i32 = 100;
pub const ID_UPDATE_BUTTON: i32 = 1;
pub const ID_LOG_EDIT: i32 = 2;
pub const ID_QUIT_BUTTON: i32 = 3;

/// Message custom: le thread de fond a poussé un ou plusieurs événements
/// dans le canal (`mpsc::Sender<WorkerMsg>`) et réveille le thread UI pour
/// qu'il les dépile. Aucune donnée n'est transportée dans le message
/// lui-même: contrairement au C++ d'origine, qui passait un pointeur vers un
/// buffer pile d'un autre thread via PostMessage (use-after-scope potentiel
/// car PostMessage est asynchrone), tout le payload transite par un canal
/// mpsc thread-safe et le message Windows ne sert que de signal de réveil.
pub const WM_APP_WORKER_EVENT: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;

/// `HWND` n'est pas `Send` par défaut (windows-rs le fait pointer vers
/// `*mut c_void`), alors qu'un handle de fenêtre Win32 est en réalité une
/// simple valeur numérique opaque, valable depuis n'importe quel thread tant
/// qu'on ne fait que la transmettre à des appels d'API (`PostMessageW`,
/// `SendMessageW`, ...) — seule la procédure de fenêtre elle-même est liée
/// au thread qui a créé la fenêtre, pas la valeur du handle. On enveloppe
/// donc `HWND` pour pouvoir la déplacer dans les threads de fond (lecture de
/// sortie winget, annulation) sans jamais la déréférencer comme pointeur.
#[derive(Clone, Copy)]
pub struct SendHwnd(windows::Win32::Foundation::HWND);

impl SendHwnd {
    pub fn new(hwnd: windows::Win32::Foundation::HWND) -> Self {
        Self(hwnd)
    }

    /// Accès volontairement via une méthode plutôt qu'un champ public: la
    /// capture disjointe des fermetures (Rust 2021) capturerait sinon
    /// directement le champ `HWND` (non-`Send`) au lieu du wrapper entier,
    /// ce qui ferait échouer silencieusement la garantie apportée par ce
    /// type dans un `std::thread::spawn(move || ...)`.
    pub fn get(&self) -> windows::Win32::Foundation::HWND {
        self.0
    }
}

// SAFETY: voir le commentaire du type; un HWND n'est jamais déréférencé,
// seulement transmis à des fonctions Win32 qui acceptent des appels
// inter-thread par conception (PostMessage en particulier est explicitement
// documentée comme thread-safe).
unsafe impl Send for SendHwnd {}

//! Localisation et exécution de `winget`, sans jamais demander l'élévation
//! UAC et sans dépendre d'une recherche de PATH implicite et non fiable.
//!
//! ## Où se trouve winget sur Windows 11 ?
//!
//! `winget` est distribué par le paquet "App Installer" (Microsoft.DesktopAppInstaller).
//! Le binaire réel vit sous un chemin versionné et imprévisible
//! (`C:\Program Files\WindowsApps\Microsoft.DesktopAppInstaller_<version>_...\winget.exe`,
//! un dossier protégé par ACL et remplacé à chaque mise à jour du paquet).
//! Ce chemin ne doit jamais être codé en dur.
//!
//! Windows expose en revanche un "alias d'exécution" stable et non privilégié:
//! `%LOCALAPPDATA%\Microsoft\WindowsApps\winget.exe`. Ce dossier est ajouté
//! automatiquement au PATH *utilisateur* (pas administrateur) par le système
//! et reste le même quelle que soit la version de winget installée. C'est la
//! voie officielle pour invoquer winget sans droits élevés, et c'est ce que
//! l'ancienne version en C++ obtenait déjà implicitement (elle laissait
//! `CreateProcessW` chercher "winget" sur le PATH) — mais elle exigeait en
//! plus `requireAdministrator` dans son manifeste, ce qui forçait UAC pour
//! *tout* le programme sans aucune raison technique.
//!
//! On résout donc ce chemin explicitement, en priorité, puis on retombe sur
//! une recherche manuelle du PATH utilisateur *uniquement* (jamais le
//! répertoire courant) si jamais l'alias n'existe pas. On n'utilise
//! délibérément jamais la recherche implicite de `CreateProcessW` (quand
//! `lpApplicationName` est NULL): elle inclut le répertoire courant du
//! processus dans son ordre de recherche, ce qui expose à un détournement de
//! binaire (CWE-427) si l'utilisateur lance `la_meuh.exe` depuis un dossier
//! contenant un `winget.exe` malveillant (ex: un dossier Téléchargements).
//! `CreateProcessW` avec un `lpApplicationName` absolu ne fait, lui, aucune
//! recherche.

use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows::Win32::System::Environment::GetEnvironmentVariableW;

/// Taille de buffer généreuse pour lire une variable d'environnement:
/// bien au-delà de tout chemin Windows réaliste (la limite légale est
/// 32767 caractères larges pour un chemin étendu).
const ENV_BUF_LEN: usize = 32 * 1024;

/// Cherche winget sans jamais toucher au répertoire courant ni demander
/// d'élévation. Retourne le chemin absolu du binaire si trouvé.
pub fn locate_winget() -> Option<PathBuf> {
    if let Some(local_app_data) = get_env_var("LOCALAPPDATA") {
        let candidate = Path::new(&local_app_data)
            .join("Microsoft")
            .join("WindowsApps")
            .join("winget.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // Repli: recherche manuelle, restreinte aux répertoires listés dans la
    // variable PATH de l'utilisateur (jamais "." ni le répertoire courant).
    if let Some(path_var) = get_env_var("PATH") {
        for dir in path_var.split(';') {
            let dir = dir.trim();
            if dir.is_empty() || dir == "." {
                continue;
            }
            let candidate = Path::new(dir).join("winget.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

fn get_env_var(name: &str) -> Option<String> {
    let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut buf = vec![0u16; ENV_BUF_LEN];
    // SAFETY: `wide_name` et `buf` sont des buffers valides, non-nuls et
    // correctement dimensionnés pour toute la durée de l'appel FFI.
    let len = unsafe {
        GetEnvironmentVariableW(
            windows::core::PCWSTR(wide_name.as_ptr()),
            Some(&mut buf[..]),
        )
    };
    if len == 0 || len as usize >= buf.len() {
        return None;
    }
    let os_string = OsString::from_wide(&buf[..len as usize]);
    os_string.into_string().ok()
}

/// Construit la ligne de commande complète (argv[0] compris) pour lancer
/// winget en mode "mise à jour de tout, sans confirmation interactive".
/// Les arguments sont tous statiques (aucune entrée utilisateur n'y est
/// jamais interpolée), donc aucun risque d'injection de commande.
pub fn build_command_line(winget_path: &Path) -> Vec<u16> {
    let quoted = format!("\"{}\"", winget_path.display());
    let full = format!(
        "{quoted} upgrade --all --accept-source-agreements --accept-package-agreements --disable-interactivity"
    );
    full.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Convertit un chemin Windows en buffer large null-terminé, pour
/// `lpApplicationName`.
pub fn to_wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_command_line_quotes_path_and_contains_expected_flags() {
        let path = PathBuf::from(r"C:\Users\Test\AppData\Local\Microsoft\WindowsApps\winget.exe");
        let cmdline = build_command_line(&path);
        let s = String::from_utf16(&cmdline[..cmdline.len() - 1]).unwrap();
        assert!(s.starts_with('"'));
        assert!(s.contains("WindowsApps\\winget.exe\" upgrade"));
        assert!(s.contains("--accept-source-agreements"));
        assert!(s.contains("--accept-package-agreements"));
        assert!(!s.contains("..\\")); // pas de traversal exotique
    }
}

# 🐄 La Meuh (Rust) — refonte sécurisée

Réécriture en Rust de [La Meuh](../La_meuh/) (originellement en C++/Win32),
un utilitaire pour lancer `winget upgrade --all` en un clic, avec la même
interface graphique et la même vache qu'avant.

Le C++ original reste dans `/home/user/La_meuh/` à titre de référence; ce
dossier ne le modifie jamais.

## Ce qui a changé, et pourquoi

### 1. Plus de demande d'élévation UAC (`requireAdministrator` → `asInvoker`)

`winget` est distribué par le paquet "App Installer". Son binaire réel vit
sous un dossier versionné et protégé par ACL
(`C:\Program Files\WindowsApps\Microsoft.DesktopAppInstaller_<version>...\`),
mais Windows expose un **alias d'exécution stable et non privilégié**:

```
%LOCALAPPDATA%\Microsoft\WindowsApps\winget.exe
```

Ce dossier est ajouté automatiquement au PATH *utilisateur* (pas
administrateur). L'ancien code C++ s'appuyait déjà implicitement dessus (il
laissait `CreateProcessW` chercher "winget" sur le PATH) — mais son
manifeste demandait quand même `requireAdministrator`, ce qui forçait UAC
pour lancer *tout le programme*, sans raison technique. La version Rust
résout ce chemin explicitement (voir [`src/winget.rs`](src/winget.rs)) et le
manifeste demande désormais `asInvoker`: aucune élévation n'est jamais
demandée par La Meuh elle-même. Si une mise à jour précise nécessite des
droits admin, c'est `winget`/le paquet concerné qui gère sa propre
élévation, paquet par paquet — principe du moindre privilège.

Autre différence volontaire: on ne réutilise jamais la recherche de PATH
*implicite* de `CreateProcessW` (celle qui s'active quand `lpApplicationName`
est NULL), car elle inclut le répertoire courant du processus dans son ordre
de recherche — un utilisateur qui double-clique `la_meuh.exe` depuis un
dossier Téléchargements contenant un faux `winget.exe` pourrait exécuter du
code arbitraire (CWE-427, "uncontrolled search path element"). La résolution
Rust ne regarde jamais le répertoire courant, et `CreateProcessW` est appelé
avec un `lpApplicationName` absolu (donc sans recherche du tout).

### 2. Cible de compilation `x86_64-pc-windows-msvc` via `cargo-xwin`

Cross-compilé depuis Linux avec [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin),
qui télécharge le CRT/Windows SDK nécessaires et pilote `clang`/`lld-link`
pour produire un binaire **ABI MSVC**, plutôt que la cible `-gnu` (mingw)
qu'utilisait le `compile.bat` d'origine. Les binaires mingw-w64
statiquement liés sont historiquement plus souvent associés à des faux
positifs antivirus que les binaires MSVC — c'est le levier le plus direct
pour réduire les alertes AV sans recourir à la signature de code (que ce
projet n'utilise pas, par choix).

Pas de packer (UPX ou autre): un exécutable "compressé/empaqueté" est lui
aussi un signal fort pour les heuristiques antivirus. Le binaire release est
juste strippé (symboles retirés) via le profil Cargo standard, ce qui n'a
rien à voir avec du packing.

### 3. Icône et logo de la vache: identiques

`resources/la_meuh.ico` et `resources/marguerite.bmp` sont des copies
directes des fichiers d'origine, embarqués via `build.rs` +
[`embed-resource`](https://crates.io/crates/embed-resource) et
`resources/la_meuh.rc`, exactement comme le faisait `windres` côté C++.

### 4. Bugs corrigés par rapport au C++ d'origine

- **Pointeur vers pile passé entre threads** (`PostMessageW(..., WM_UPDATE_LOG, ..., (LPARAM)chBuf)`
  où `chBuf` était un tableau local du thread de fond): `PostMessage` est
  asynchrone, donc la fenêtre pouvait lire ce pointeur bien après que le
  thread de fond ait réutilisé/quitté ce buffer — use-after-scope. Remplacé
  par un canal (`mpsc::channel`) qui transporte des `String` possédées; le
  message Windows ne sert plus qu'à réveiller le thread UI.
- **État global partagé sans synchronisation** (`bUpdateInProgress`,
  `hWingetProcess` lus/écrits depuis le thread UI et le thread de mise à
  jour sans verrou): remplacé par un `AtomicBool` et un `Mutex`.
- **Fuite de handles** dans `ExecuteWingetUpgrade`: si `SetHandleInformation`
  échouait, la fonction retournait sans fermer les deux bouts du pipe déjà
  créés. Remplacé par une enveloppe RAII (`HandleGuard`) qui ferme toujours
  le handle, y compris sur les chemins d'erreur anticipés.
- **Coupure UTF-8 à la frontière de lecture**: `ReadFile` peut couper une
  séquence UTF-8 multi-octets pile entre deux appels; l'original décodait
  chaque bloc de 4096 octets indépendamment, ce qui pouvait corrompre le
  dernier caractère d'un bloc. La version Rust recolle les octets
  incomplets avant de les redécoder au tour suivant.
- **Arrêt brutal de winget** (`TerminateProcess` immédiat au clic sur
  Quitter): peut interrompre une installation/désinstallation en plein vol
  et laisser un paquet dans un état incohérent. La version Rust envoie
  d'abord un `CTRL_BREAK_EVENT` au groupe de processus de winget (démarré
  avec `CREATE_NEW_PROCESS_GROUP`) et attend 5 secondes avant de recourir à
  `TerminateProcess` en dernier ressort.
- **Partage non sûr du HANDLE de processus** entre le thread de lecture de
  sortie et le mécanisme d'annulation: les deux auraient pu fermer "le
  même" HANDLE pendant que l'autre l'utilisait encore. Chacun reçoit
  maintenant sa propre copie via `DuplicateHandle`.

## Structure

```
la_meuh_rust/
├── Cargo.toml
├── build.rs                 # embarque icône/bitmap/manifest/version-info
├── .cargo/config.toml       # cible msvc + runner Wine pour `cargo test`
├── resources/
│   ├── la_meuh.ico          # copie de l'original
│   ├── marguerite.bmp       # copie de l'original
│   ├── la_meuh.manifest     # asInvoker (plus de requireAdministrator)
│   └── la_meuh.rc
├── src/
│   ├── main.rs               # point d'entrée, classe de fenêtre, boucle de messages
│   ├── app.rs                 # état de la fenêtre + WndProc
│   ├── process.rs            # lancement winget, lecture de sortie, annulation propre
│   ├── winget.rs              # résolution sécurisée du chemin de winget
│   └── resources.rs
├── docker/
│   ├── Dockerfile            # image Debian: rustup, cargo-xwin, wine, clippy, audit, geiger
│   └── run_pipeline.sh       # fmt + clippy + audit + geiger + tests (Wine) + build + fumée
└── target/x86_64-pc-windows-msvc/release/la_meuh.exe   # binaire final
```

## Build & tests (toujours en Docker, jamais sur l'hôte)

```bash
docker build -f docker/Dockerfile -t la-meuh-rust-builder:debian .
docker run --rm --user "$(id -u):$(id -g)" -v "$(pwd)":/build la-meuh-rust-builder:debian ./docker/run_pipeline.sh
```

`--user "$(id -u):$(id -g)"` est important: sans ça, le conteneur tourne en
root et tous les fichiers qu'il écrit dans ce dossier (`target/`,
`Cargo.lock`...) appartiennent à root sur la machine hôte, ce qui bloque
ensuite leur suppression/copie/déplacement par un gestionnaire de fichiers
normal (Nautilus, Thunar, `rm` sans sudo...).

Le pipeline lance `cargo fmt --check`, `cargo clippy -D warnings`,
`cargo audit`, `cargo geiger`, la suite de tests sous Wine, puis la
cross-compilation release et un test de fumée (lancement du binaire sous
Wine + Xvfb pour vérifier qu'il démarre sans planter).

## Résultats de la dernière exécution du pipeline

- `cargo fmt --check`: OK
- `cargo clippy --all-targets -D warnings`: OK, aucun avertissement
- `cargo audit`: OK, 0 vulnérabilité connue sur 54 dépendances
- `cargo geiger`: rapport généré — le code de `la_meuh` est entièrement
  `unsafe` en surface (inévitable pour des appels Win32 bruts), concentré
  dans `process.rs`/`app.rs`/`winget.rs` et documenté (commentaires
  `SAFETY:`); les enveloppes RAII (`HandleGuard`) et `Send` explicites
  couvrent les seuls points où une durée de vie/thread-safety implicite
  aurait pu introduire un bug
- Tests unitaires (`cargo xwin test`, exécutés sous Wine): 1/1 passés
- `cargo xwin build --release`: OK — binaire MSVC, `PE32+ (GUI), x86-64,
  7 sections`, ~1.8 Mo (contre 2.2 Mo / 19 sections pour l'original mingw)
- Fumée sous Wine + Xvfb (headless): le processus tourne toujours après 4s
  (fenêtre créée, boucle de messages active, pas de crash immédiat). Wine
  n'ayant pas de vrai `winget`, le scénario "mise à jour réussie" n'a pas pu
  être testé de bout en bout ici — seul un vrai Windows 11 le permettra.

## Licence

MIT — © 2026 Spellskite-coding et Marwane Toury.

# 🐄 La Meuh (Rust) — secure rewrite

Rust rewrite of [La Meuh](../La_meuh/) (originally in C++/Win32), a utility
for launching `winget upgrade --all` with a single click, with the same
graphical interface and the same cow as before.

The original C++ code stays in `/home/user/La_meuh/` for reference; this
folder never modifies it.

## What changed, and why

### 1. No more UAC elevation prompt (`requireAdministrator` → `asInvoker`)

`winget` is shipped as part of the "App Installer" package. Its actual
binary lives under a versioned, ACL-protected folder
(`C:\Program Files\WindowsApps\Microsoft.DesktopAppInstaller_<version>...\`),
but Windows exposes a **stable, non-privileged execution alias**:

```
%LOCALAPPDATA%\Microsoft\WindowsApps\winget.exe
```

This folder is automatically added to the *user* PATH (not the
administrator one). The original C++ code already relied on it implicitly
(it let `CreateProcessW` search for "winget" on the PATH) — but its
manifest still requested `requireAdministrator`, which forced UAC to launch
*the entire program*, for no technical reason. The Rust version resolves
this path explicitly (see [`src/winget.rs`](src/winget.rs)) and the
manifest now requests `asInvoker`: La Meuh itself never requests elevation.
If a specific update requires admin rights, it's `winget`/the package in
question that handles its own elevation, package by package — principle of
least privilege.

Another deliberate difference: the *implicit* PATH search of
`CreateProcessW` (the one that kicks in when `lpApplicationName` is NULL)
is never reused, because it includes the process's current directory in its
search order — a user double-clicking `la_meuh.exe` from a Downloads folder
containing a fake `winget.exe` could end up executing arbitrary code
(CWE-427, "uncontrolled search path element"). The Rust resolution never
looks at the current directory, and `CreateProcessW` is called with an
absolute `lpApplicationName` (so no search happens at all).

### 2. `x86_64-pc-windows-msvc` compilation target via `cargo-xwin`

Cross-compiled from Linux with [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin),
which downloads the required CRT/Windows SDK and drives `clang`/`lld-link`
to produce an **MSVC ABI** binary, rather than the `-gnu` (mingw) target
that the original `compile.bat` used. Statically linked mingw-w64 binaries
have historically been flagged as antivirus false positives more often
than MSVC binaries — this is the most direct lever for reducing AV alerts
without resorting to code signing (which this project doesn't use, by
choice).

No packer (UPX or otherwise): a "compressed/packed" executable is also a
strong signal for antivirus heuristics. The release binary is simply
stripped (symbols removed) via the standard Cargo profile, which has
nothing to do with packing.

### 3. Cow icon and logo: identical

`resources/la_meuh.ico` and `resources/marguerite.bmp` are direct copies of
the original files, embedded via `build.rs` +
[`embed-resource`](https://crates.io/crates/embed-resource) and
`resources/la_meuh.rc`, exactly as `windres` did on the C++ side.

### 4. Bugs fixed compared to the original C++

- **Stack pointer passed between threads** (`PostMessageW(..., WM_UPDATE_LOG, ..., (LPARAM)chBuf)`
  where `chBuf` was a local array of the background thread): `PostMessage`
  is asynchronous, so the window could read this pointer well after the
  background thread had reused/exited that buffer — use-after-scope.
  Replaced with a channel (`mpsc::channel`) that carries owned `String`
  values; the Windows message now only serves to wake up the UI thread.
- **Shared global state without synchronization** (`bUpdateInProgress`,
  `hWingetProcess` read/written from both the UI thread and the update
  thread without a lock): replaced with an `AtomicBool` and a `Mutex`.
- **Handle leak** in `ExecuteWingetUpgrade`: if `SetHandleInformation`
  failed, the function returned without closing the two already-created
  pipe ends. Replaced with an RAII wrapper (`HandleGuard`) that always
  closes the handle, including on early error paths.
- **UTF-8 cut at the read boundary**: `ReadFile` can split a multi-byte
  UTF-8 sequence right between two calls; the original decoded each 4096-byte
  chunk independently, which could corrupt the last character of a chunk.
  The Rust version stitches incomplete bytes back together before
  redecoding them on the next pass.
- **Abrupt winget termination** (immediate `TerminateProcess` on clicking
  Quit): could interrupt an install/uninstall mid-flight and leave a
  package in an inconsistent state. The Rust version first sends a
  `CTRL_BREAK_EVENT` to winget's process group (started with
  `CREATE_NEW_PROCESS_GROUP`) and waits 5 seconds before falling back to
  `TerminateProcess` as a last resort.
- **Unsafe sharing of the process HANDLE** between the output-reading
  thread and the cancellation mechanism: both could have closed "the same"
  HANDLE while the other was still using it. Each now receives its own
  copy via `DuplicateHandle`.

## Structure

```
la_meuh_rust/
├── Cargo.toml
├── build.rs                 # embeds icon/bitmap/manifest/version-info
├── .cargo/config.toml       # msvc target + Wine runner for `cargo test`
├── resources/
│   ├── la_meuh.ico          # copy of the original
│   ├── marguerite.bmp       # copy of the original
│   ├── la_meuh.manifest     # asInvoker (no more requireAdministrator)
│   └── la_meuh.rc
├── src/
│   ├── main.rs               # entry point, window class, message loop
│   ├── app.rs                 # window state + WndProc
│   ├── process.rs            # winget launch, output reading, clean cancellation
│   ├── winget.rs              # secure resolution of winget's path
│   └── resources.rs
├── docker/
│   ├── Dockerfile            # Debian image: rustup, cargo-xwin, wine, clippy, audit, geiger
│   └── run_pipeline.sh       # fmt + clippy + audit + geiger + tests (Wine) + build + smoke test
└── target/x86_64-pc-windows-msvc/release/la_meuh.exe   # final binary
```

## Build & tests (always in Docker, never on the host)

```bash
docker build -f docker/Dockerfile -t la-meuh-rust-builder:debian .
docker run --rm --user "$(id -u):$(id -g)" -v "$(pwd)":/build la-meuh-rust-builder:debian ./docker/run_pipeline.sh
```

`--user "$(id -u):$(id -g)"` matters: without it, the container runs as
root and every file it writes into this folder (`target/`, `Cargo.lock`...)
ends up owned by root on the host machine, which then blocks their
deletion/copy/move by a regular file manager (Nautilus, Thunar, `rm`
without sudo...).

The pipeline runs `cargo fmt --check`, `cargo clippy -D warnings`,
`cargo audit`, `cargo geiger`, the test suite under Wine, then the release
cross-compilation and a smoke test (launching the binary under Wine + Xvfb
to check it starts without crashing).

## Results from the last pipeline run

- `cargo fmt --check`: OK
- `cargo clippy --all-targets -D warnings`: OK, no warnings
- `cargo audit`: OK, 0 known vulnerabilities across 54 dependencies
- `cargo geiger`: report generated — `la_meuh`'s code is entirely `unsafe`
  on the surface (unavoidable for raw Win32 calls), concentrated in
  `process.rs`/`app.rs`/`winget.rs` and documented (`SAFETY:` comments);
  the RAII wrappers (`HandleGuard`) and explicit `Send` cover the only
  points where an implicit lifetime/thread-safety issue could have
  introduced a bug
- Unit tests (`cargo xwin test`, run under Wine): 1/1 passed
- `cargo xwin build --release`: OK — MSVC binary, `PE32+ (GUI), x86-64,
  7 sections`, ~1.8 MB (vs. 2.2 MB / 19 sections for the original mingw
  build)
- Smoke test under Wine + Xvfb (headless): the process is still running
  after 4s (window created, message loop active, no immediate crash).
  Since Wine doesn't have a real `winget`, the "successful update"
  scenario couldn't be fully tested end-to-end here — only a real Windows
  11 machine can do that.

## License

MIT — © 2026 Spellskite-coding and Marwane Toury.

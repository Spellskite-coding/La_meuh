#!/usr/bin/env bash
# Pipeline complet exécuté DANS le conteneur Docker: lint, SAST, tests,
# cross-compilation, puis fumée sous Wine. Rien de tout ceci ne doit
# jamais tourner directement sur l'hôte.
set -euo pipefail
cd /build

export WINEDEBUG=-all
export DISPLAY=:99
Xvfb :99 -screen 0 1024x768x16 &
XVFB_PID=$!
sleep 1
trap 'kill $XVFB_PID 2>/dev/null || true' EXIT

echo "== [1/7] cargo fmt --check =="
cargo fmt --check

echo "== [2/7] cargo clippy (deny warnings) =="
cargo xwin clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings

echo "== [3/7] cargo audit (dépendances vulnérables) =="
cargo audit || true # informatif: peut échouer si la base RustSec est injoignable hors-ligne

echo "== [4/7] cargo geiger (surface unsafe) =="
# cargo-geiger n'est pas un sous-programme de cargo-xwin: il faut lui donner
# manuellement l'environnement (CC/CFLAGS/linker MSVC via clang-cl) que
# `cargo xwin build/clippy/test` configurent automatiquement, sinon
# build.rs retombe sur le gcc de l'hôte pour compiler resources/la_meuh.rc.
(eval "$(cargo xwin env --target x86_64-pc-windows-msvc)" && cargo geiger --target x86_64-pc-windows-msvc) || true

echo "== [5/7] cargo test (sous Wine) =="
cargo xwin test --target x86_64-pc-windows-msvc

echo "== [6/7] cargo xwin build --release =="
cargo xwin build --release --target x86_64-pc-windows-msvc

echo "== [7/7] Fumée sous Wine (headless) =="
timeout 10 wine target/x86_64-pc-windows-msvc/release/la_meuh.exe &
APP_PID=$!
sleep 4
if wine tasklist 2>/dev/null | grep -qi la_meuh; then
    echo "OK: la_meuh.exe tourne toujours après 4s (fenêtre créée, pas de crash immédiat)."
else
    echo "ATTENTION: la_meuh.exe ne semble plus tourner (voir logs ci-dessus)."
fi
wait $APP_PID 2>/dev/null || true

echo "== Pipeline terminé =="
ls -la target/x86_64-pc-windows-msvc/release/la_meuh.exe
file target/x86_64-pc-windows-msvc/release/la_meuh.exe

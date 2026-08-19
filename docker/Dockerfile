# Environnement de cross-compilation + test pour La Meuh (Rust)
# Base: debian:stable-slim (déjà présente localement)
FROM debian:stable-slim

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:$PATH

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates curl git build-essential pkg-config \
        clang lld llvm \
        wine64 xvfb xauth \
        unzip xz-utils \
        libssl-dev \
        file \
        python3 \
    && rm -rf /var/lib/apt/lists/*

# Rust stable + cible MSVC pour cross-compilation via cargo-xwin
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --profile minimal --default-toolchain stable \
        --component clippy --component rustfmt
RUN rustup target add x86_64-pc-windows-msvc

# Le paquet Debian wine64 n'installe pas de binaire `wine` sur le PATH par
# défaut (juste /usr/lib/wine/wine64); or `cargo xwin test`/`run` injecte
# CARGO_TARGET_..._RUNNER=wine (qui prime sur .cargo/config.toml). Un simple
# symlink nommé "wine" perturbe la détection d'argv[0] de Wine ("could not
# exec the wine loader"); un petit wrapper shell qui `exec` le vrai binaire
# évite le problème.
RUN printf '#!/bin/sh\nexec /usr/lib/wine/wine64 "$@"\n' > /usr/local/bin/wine \
    && chmod +x /usr/local/bin/wine

# Outils de build cross + audit / SAST
RUN cargo install --locked cargo-xwin cargo-audit cargo-geiger

ENV XWIN_CACHE_DIR=/opt/xwin-cache

# Pré-télécharge et met en cache le Windows SDK/CRT (xwin) dans l'image,
# via un projet jetable, pour que les builds réels (montés en volume) soient
# rapides et n'aient pas besoin du réseau à l'exécution.
RUN cargo new --bin /tmp/warm --vcs none \
    && cd /tmp/warm \
    && cargo xwin build --release --target x86_64-pc-windows-msvc \
    && rm -rf /tmp/warm

# Le conteneur tourne par défaut en root, ce qui fait que tout fichier écrit
# dans /build (le dossier du projet, monté depuis l'hôte) appartient à root
# sur la machine hôte — inutilisable ensuite par l'utilisateur normal
# (Nautilus/Thunar/`rm` échouent avec "permission non accordée"). Toujours
# lancer ce conteneur avec `docker run --user "$(id -u):$(id -g)" ...` pour
# que les fichiers créés (target/, Cargo.lock, etc.) appartiennent à
# l'utilisateur hôte. Ça suppose que cet utilisateur puisse écrire dans les
# répertoires Rust de l'image (toolchain, registre cargo, cache xwin), d'où
# ce chmod: l'image elle-même reste root-owned, mais devient accessible en
# écriture à n'importe quel UID.
RUN chmod -R a+rwX /opt/rustup /opt/cargo /opt/xwin-cache

WORKDIR /build
CMD ["/bin/bash"]

# Build box for KidDOS's Linux and Windows binaries. Runs anywhere Docker
# runs; no hosted CI involved. See tools/release-docker.sh.
#
# The image's own architecture builds its Linux target natively; the other
# Linux architecture is cross-compiled with Debian's multiarch packages;
# Windows is cross-compiled with MinGW.
FROM rust:1-bookworm
RUN set -e; HOST=$(dpkg --print-architecture); \
    if [ "$HOST" = arm64 ]; then OTHER=amd64; CROSS=gcc-x86-64-linux-gnu; else OTHER=arm64; CROSS=gcc-aarch64-linux-gnu; fi; \
    dpkg --add-architecture $OTHER; apt-get update; \
    apt-get install -y --no-install-recommends pkg-config zip mingw-w64 \
      libasound2-dev libxkbcommon-dev \
      $CROSS libc6-dev:$OTHER libasound2-dev:$OTHER libxkbcommon-dev:$OTHER; \
    rm -rf /var/lib/apt/lists/*
RUN rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-pc-windows-gnu
# cross linkers and C compilers (rusqlite builds SQLite from C). The
# native target's gcc is plain gcc; the other one's is the cross gcc.
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
    CC_x86_64_unknown_linux_gnu=x86_64-linux-gnu-gcc \
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc \
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
    CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc \
    PKG_CONFIG_ALLOW_CROSS=1
# Debian's native gcc is also reachable under its triplet name, so the
# variables above are right on either host once these links exist.
RUN for t in x86_64-linux-gnu aarch64-linux-gnu; do [ -x /usr/bin/$t-gcc ] || ln -s /usr/bin/gcc /usr/bin/$t-gcc; done
WORKDIR /src

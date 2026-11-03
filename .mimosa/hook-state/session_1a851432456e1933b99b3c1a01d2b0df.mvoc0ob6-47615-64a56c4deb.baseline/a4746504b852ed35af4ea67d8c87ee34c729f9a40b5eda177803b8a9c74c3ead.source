#!/usr/bin/env bash
# T-890-08 (G-40): empacota o Studio GUI como .deb.
#
# Uso: scripts/build-studio-deb.sh [versão]
#
# - Compila o binário release (feature dds) se necessário.
# - Monta a árvore debian em um diretório temporário: /usr/bin/studio,
#   /usr/share/applications/dds-orchestrator-studio.desktop e
#   /usr/share/doc/dds-orchestrator-studio/copyright.
# - Gera dist/dds-orchestrator-studio_<versão>_amd64.deb (fora do git).
# - Valida por EXTRAÇÃO (não instala no sistema — instalar é decisão do
#   autor: sudo dpkg -i dist/<pacote>.deb).
set -euo pipefail

VERSION="${1:-0.1.0}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Target FORA da árvore (SMB não suporta symlink — regra operacional do
# repo) + build estático do CycloneDDS (CYCLONEDDS_STATIC=1).
TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/tese-rust-target-fd}"
BIN="$TARGET_DIR/release/studio"
OUT="$ROOT/dist"

echo "== [1/4] binário release (feature dds, CycloneDDS estático) =="
CARGO_TARGET_DIR="$TARGET_DIR" CYCLONEDDS_STATIC=1 \
  cargo build --release -p orchestrator-studio --features dds
[[ -x "$BIN" ]] || { echo "binário não encontrado: $BIN"; exit 1; }

# Depende honesta: maior GLIBC exigida pelo binário construído.
GLIBC_REQ="$(objdump -T "$BIN" | grep -oE 'GLIBC_2\.[0-9]+' | sort -uV | tail -1 | sed 's/GLIBC_//')"

echo "== [2/4] árvore debian (versão $VERSION, libc6 >= $GLIBC_REQ) =="
DEB="$(mktemp -d)/dds-orchestrator-studio"
mkdir -p "$DEB/DEBIAN" \
         "$DEB/usr/bin" \
         "$DEB/usr/share/applications" \
         "$DEB/usr/share/doc/dds-orchestrator-studio"

cp "$BIN" "$DEB/usr/bin/studio"
chmod 755 "$DEB/usr/bin/studio"
cp "$ROOT/packaging/desktop/dds-orchestrator-studio.desktop" \
   "$DEB/usr/share/applications/dds-orchestrator-studio.desktop"

cat > "$DEB/usr/share/doc/dds-orchestrator-studio/copyright" <<EOF
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: dds-orchestrator-studio
Source: repositório da tese DDS-LLM-Orchestrator (src/rust)

Files: *
Copyright: 2026 Matheus Zeitune (UERJ)
License: MIT
 Permission is hereby granted, free of charge, to any person obtaining a
 copy of this software and associated documentation files (the "Software"),
 to deal in the Software without restriction, including without limitation
 the rights to use, copy, modify, merge, publish, distribute, sublicense,
 and/or sell copies of the Software, and to permit persons to whom the
 Software is furnished to do so, subject to the following conditions:
 .
 The above copyright notice and this permission notice shall be included
 in all copies or substantial portions of the Software.
EOF

INSTALLED_SIZE="$(du -sk "$DEB/usr" | cut -f1)"
cat > "$DEB/DEBIAN/control" <<EOF
Package: dds-orchestrator-studio
Version: $VERSION
Section: devel
Priority: optional
Architecture: amd64
Installed-Size: $INSTALLED_SIZE
Depends: libc6 (>= $GLIBC_REQ)
Maintainer: DDS-LLM Orchestrator Project
Description: Desktop do DDS Orchestrator Studio (tese UERJ)
 GUI egui para administração do enxame DDS-LLM: catálogo compartilhado,
 máquinas (studio-node), inferência, agentes, ferramentas, workflow
 A→B→C e topologia DDS ao vivo (19 tópicos canônicos).
EOF

echo "== [3/4] montagem do .deb =="
mkdir -p "$OUT"
PKG="$OUT/dds-orchestrator-studio_${VERSION}_amd64.deb"
# md5sums (recomendado pelo Debian para integridade do payload)
(cd "$DEB" && find usr -type f -exec md5sum {} \; ) > "$DEB/DEBIAN/md5sums"
if command -v dpkg-deb >/dev/null 2>&1; then
  dpkg-deb --build --root-owner-group "$DEB" "$PKG"
else
  # Fedora sem dpkg-dev: monta o .deb NA MÃO — um ar com debian-binary +
  # control.tar.gz + data.tar.xz (é exatamente o formato que o dpkg-deb
  # produz; ar/tar/xz estão no binutils/tar base).
  STAGE="$(mktemp -d)"
  tar -C "$DEB/DEBIAN" -czf "$STAGE/control.tar.gz" control md5sums
  tar -C "$DEB" -cJf "$STAGE/data.tar.xz" usr
  printf '2.0\n' > "$STAGE/debian-binary"
  (cd "$STAGE" && ar -r "$PKG" debian-binary control.tar.gz data.tar.xz)
  rm -rf "$STAGE"
fi

echo "== [4/4] validação por EXTRAÇÃO (não instala) =="
CHECK="$(mktemp -d)"
if command -v dpkg-deb >/dev/null 2>&1; then
  dpkg-deb --info "$PKG" | sed -n '1,12p'
  dpkg-deb -x "$PKG" "$CHECK"
else
  ar t "$PKG"
  STAGE2="$(mktemp -d)"
  (cd "$STAGE2" && ar x "$PKG" data.tar.xz && tar -C "$CHECK" -xJf data.tar.xz)
  rm -rf "$STAGE2"
fi
test -x "$CHECK/usr/bin/studio" || { echo "FALHA: binário ausente no payload"; exit 1; }
diff "$ROOT/packaging/desktop/dds-orchestrator-studio.desktop" \
     "$CHECK/usr/share/applications/dds-orchestrator-studio.desktop" \
  && echo "desktop idêntico ✓"
if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "$CHECK/usr/share/applications/dds-orchestrator-studio.desktop" \
    && echo "desktop-file-validate ✓"
fi
rm -rf "$CHECK" "$(dirname "$DEB")"
echo "OK: $PKG"

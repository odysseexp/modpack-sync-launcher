#!/usr/bin/env bash
set -euo pipefail

# ==============================================================================
# Script de Setup e Compilação Local para SteamOS (via Distrobox Arch Linux)
# Uso dentro do container Arch:
#   distrobox enter Arch -- bash ./scripts/distrobox-arch-build.sh [dev|build]
# ==============================================================================

export PATH="/usr/bin:$PATH"
MODE="${1:-dev}"

echo "==> [0/4] Verificando integridade do banco do pacman no container Arch..."
for d in /var/lib/pacman/local/*/; do
  if [ -d "$d" ] && [ ! -f "${d}desc" ]; then
    echo "    Reparando entrada corrompida do pacman: $d"
    sudo rm -rf "$d"
  fi
done

echo "==> [1/4] Verificando pacotes Tauri v2 no Arch Linux..."
sudo pacman -S --needed --noconfirm --overwrite '*' \
  base-devel \
  curl \
  wget \
  file \
  openssl \
  appmenu-gtk-module \
  libappindicator-gtk3 \
  librsvg \
  webkit2gtk-4.1 \
  patchelf \
  nodejs \
  npm \
  rust

echo "==> [2/4] Instalando dependências NPM do frontend..."
npm install
if [ -f "node_modules/esbuild/install.js" ]; then
  node node_modules/esbuild/install.js || true
fi

if [ "$MODE" = "build" ]; then
  echo "==> [3/4] Gerando pacote .AppImage nativo para Linux..."
  NO_STRIP=true npm run tauri build -- --bundles appimage

  echo "==> [4/4] Empacotando runtime glibc 2.44 isolado para compatibilidade direta com SteamOS..."
  APPDIR="src-tauri/target/release/bundle/appimage/Modpack Sync Launcher.AppDir"
  mkdir -p "$APPDIR/usr/glibc"
  for lib in ld-linux-x86-64.so.2 libc.so.6 libm.so.6 libpthread.so.0 libdl.so.2 librt.so.1 libresolv.so.2 libgcc_s.so.1 libstdc++.so.6; do
    if [ -e "/usr/lib/$lib" ]; then
      cp -Lf "/usr/lib/$lib" "$APPDIR/usr/glibc/$lib"
    fi
  done

  WKDIR="$APPDIR/usr/lib/webkit2gtk-4.1"
  for proc in WebKitNetworkProcess WebKitWebProcess; do
    if [ -f "$WKDIR/$proc" ] && [ ! -f "$WKDIR/${proc}.bin" ]; then
      mv "$WKDIR/$proc" "$WKDIR/${proc}.bin"
    fi
    cat > "$WKDIR/$proc" << 'EOF'
#!/bin/sh
DIR="$(dirname "$(readlink -f "$0")")"
APPDIR="$(readlink -f "$DIR/../../..")"
exec "$APPDIR/usr/glibc/ld-linux-x86-64.so.2" --library-path "$APPDIR/usr/glibc:$APPDIR/usr/lib" "$DIR/$(basename "$0").bin" "$@"
EOF
    chmod +x "$WKDIR/$proc"
  done

  cat > "$APPDIR/AppRun" << 'EOF'
#!/usr/bin/env bash
set -e
this_dir="$(readlink -f "$(dirname "$0")")"
export APPDIR="$this_dir"
source "$this_dir/apprun-hooks/linuxdeploy-plugin-gtk.sh"
export WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS=1
cd "$this_dir/usr"
exec "$this_dir/usr/glibc/ld-linux-x86-64.so.2" --library-path "$this_dir/usr/glibc:$this_dir/usr/lib" "$this_dir/usr/bin/modpack-sync-launcher" "$@"
EOF
  chmod +x "$APPDIR/AppRun"

  ~/.cache/tauri/linuxdeploy-plugin-appimage.AppImage --appdir "$APPDIR"
  mv -f Modpack_Sync_Launcher-x86_64.AppImage "./Modpack-Sync-Launcher-x86_64.AppImage"
  chmod +x "./Modpack-Sync-Launcher-x86_64.AppImage"
  cp -f "./Modpack-Sync-Launcher-x86_64.AppImage" "src-tauri/target/release/bundle/appimage/Modpack Sync Launcher_1.0.0_amd64.AppImage"
  echo "==> Concluído! Executável pronto em: ./Modpack-Sync-Launcher-x86_64.AppImage"
else
  echo "==> [3/4] Iniciando Modpack Sync Launcher em modo de desenvolvimento..."
  npm run tauri dev
fi

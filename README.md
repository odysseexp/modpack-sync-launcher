# Modpack Sync Launcher

Aplicativo desktop multiplataforma (**Linux `.AppImage`**, **Windows `.exe`** e **macOS ARM64 `.dmg`**) construído com **Tauri v2 (Rust + React/TypeScript)** para distribuir, instalar e manter sincronizado seu modpack de Minecraft para seus amigos sem configuração manual.

---

## Como rodar e compilar no SteamOS (Container Arch Linux via Distrobox)

### 1. Modo de Desenvolvimento (Testar localmente no SteamOS)
No terminal do SteamOS, dentro da pasta do projeto:

```bash
distrobox enter Arch -- bash ./scripts/distrobox-arch-build.sh dev
```

### 2. Compilar o `.AppImage` localmente no Arch Distrobox
```bash
distrobox enter Arch -- bash ./scripts/distrobox-arch-build.sh build
```
O executável `.AppImage` será gerado em:
`src-tauri/target/release/bundle/appimage/`

---

## Compilação Automática para Windows (`.exe`), macOS ARM64 (`.dmg`) e Linux (`.AppImage`)
O arquivo `.github/workflows/release.yml` já está configurado para compilar nos servidores gratuitos do GitHub Actions sempre que você clicar em **"Criar Nova Versão e Publicar no GitHub"** no Modo Criador (Admin) do app:
- `ubuntu-22.04` → `.AppImage`
- `windows-latest` → `.exe` (NSIS)
- `macos-latest` (`aarch64-apple-darwin`) → `.dmg` (Apple Silicon ARM64)

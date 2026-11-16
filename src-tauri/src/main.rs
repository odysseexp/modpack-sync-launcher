// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use modpack_sync_launcher_lib::api::DEFAULT_CF_API_KEY;
use modpack_sync_launcher_lib::github::publish_new_release;
use modpack_sync_launcher_lib::scanner::scan_creator_folder;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--publish-cli") {
        let rt = tokio::runtime::Runtime::new().expect("Falha ao iniciar Tokio runtime");
        rt.block_on(async {
            let src = "/home/deck/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances/Vanilla/minecraft";
            let repo = "odysseexp/modpack-sync-launcher";
            let version = "1.0.0";
            println!("==> Escaneando pasta fonte: {}", src);
            let manifest = scan_creator_folder(src, version, true, true, DEFAULT_CF_API_KEY)
                .await
                .expect("Erro ao escanear pasta fonte");
            println!(
                "==> Escaneamento concluído! {} itens resolvidos (MC {} | {} {}).",
                manifest.items.len(),
                manifest.minecraft_version,
                manifest.loader.name,
                manifest.loader.version
            );
            let staging = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("modpack-sync-launcher")
                .join("staging");
            let res = publish_new_release(
                manifest,
                &PathBuf::from(src),
                &staging,
                repo,
                "",
                "Primeira release oficial do Modpack Sync Launcher (v1.0.0) com manifesto dos mods, configs.zip e instaladores multiplataforma (Linux .AppImage, Windows .exe e macOS ARM64 .dmg).",
            )
            .await
            .expect("Erro ao publicar release no GitHub");
            println!("==> Release publicada com sucesso: {}", res.release_url);
        });
        return;
    }

    modpack_sync_launcher_lib::run()
}

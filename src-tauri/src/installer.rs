use crate::api::build_http_client;
use crate::hasher::compute_file_hashes;
use crate::models::{
    DownloadProgressEvent, InPlaceUpdateCandidate, ItemCategory, LauncherPathCandidate,
    ModPlatform, ModpackManifest,
};
use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use serde_json::json;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

/// Retorna todos os caminhos padrão dos Launchers de acordo com o Sistema Operacional.
pub fn detect_launcher_paths(os_name: &str) -> Vec<LauncherPathCandidate> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let appdata = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join("AppData").join("Roaming"));

    let mut candidates = Vec::new();

    match os_name {
        "windows" => {
            let items = [
                (
                    "prism",
                    "default",
                    "Padrão (%APPDATA%\\PrismLauncher)",
                    appdata.join("PrismLauncher"),
                ),
                (
                    "atlauncher",
                    "default",
                    "Padrão (%APPDATA%\\ATLauncher)",
                    appdata.join("ATLauncher"),
                ),
                (
                    "official",
                    "default",
                    "Padrão (%APPDATA%\\.minecraft)",
                    appdata.join(".minecraft"),
                ),
            ];
            for (lid, fid, label, path) in items {
                candidates.push(LauncherPathCandidate {
                    launcher_id: lid.to_string(),
                    format_id: fid.to_string(),
                    label: label.to_string(),
                    exists: path.exists(),
                    resolved_path: path.to_string_lossy().to_string(),
                });
            }
        }
        "macos" => {
            let app_support = home.join("Library").join("Application Support");
            let items = [
                (
                    "prism",
                    "default",
                    "Padrão (~/Library/Application Support/PrismLauncher)",
                    app_support.join("PrismLauncher"),
                ),
                (
                    "atlauncher",
                    "default",
                    "Padrão (~/Library/Application Support/ATLauncher)",
                    app_support.join("ATLauncher"),
                ),
                (
                    "official",
                    "default",
                    "Padrão (~/Library/Application Support/minecraft)",
                    app_support.join("minecraft"),
                ),
            ];
            for (lid, fid, label, path) in items {
                candidates.push(LauncherPathCandidate {
                    launcher_id: lid.to_string(),
                    format_id: fid.to_string(),
                    label: label.to_string(),
                    exists: path.exists(),
                    resolved_path: path.to_string_lossy().to_string(),
                });
            }
        }
        _ => {
            // Linux (SteamOS / Arch / GNOME / KDE)
            let items = [
                (
                    "prism",
                    "flatpak",
                    "Flatpak (~/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher)",
                    home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher"),
                ),
                (
                    "prism",
                    "native",
                    "AppImage / Nativo (~/.local/share/PrismLauncher)",
                    home.join(".local/share/PrismLauncher"),
                ),
                (
                    "atlauncher",
                    "flatpak",
                    "Flatpak (~/.var/app/com.atlauncher.ATLauncher/data/ATLauncher)",
                    home.join(".var/app/com.atlauncher.ATLauncher/data/ATLauncher"),
                ),
                (
                    "atlauncher",
                    "native",
                    "AppImage / Nativo (~/.local/share/ATLauncher)",
                    home.join(".local/share/ATLauncher"),
                ),
                (
                    "official",
                    "flatpak",
                    "Flatpak (~/.var/app/com.mojang.Minecraft/.minecraft)",
                    home.join(".var/app/com.mojang.Minecraft/.minecraft"),
                ),
                (
                    "official",
                    "native",
                    "AppImage / Nativo (~/.minecraft)",
                    home.join(".minecraft"),
                ),
            ];
            for (lid, fid, label, path) in items {
                candidates.push(LauncherPathCandidate {
                    launcher_id: lid.to_string(),
                    format_id: fid.to_string(),
                    label: label.to_string(),
                    exists: path.exists(),
                    resolved_path: path.to_string_lossy().to_string(),
                });
            }
        }
    }

    candidates
}

/// Configura a instância no Launcher selecionado (PrismLauncher, ATLauncher ou Oficial)
/// e retorna o caminho exato da pasta `minecraft` onde `mods/`, `resourcepacks/`, `shaderpacks/` e `config/` ficam.
pub fn configure_launcher_instance(
    launcher_id: &str,
    base_launcher_dir: &Path,
    instance_name: &str,
    manifest: &ModpackManifest,
) -> Result<PathBuf> {
    let safe_folder_name: String = instance_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    match launcher_id {
        "prism" => {
            // Se o usuário apontou diretamente para uma pasta `minecraft` de instância existente, respeita
            let instance_root = if base_launcher_dir.ends_with("minecraft")
                && base_launcher_dir
                    .parent()
                    .map(|p| p.join("mmc-pack.json").exists())
                    .unwrap_or(false)
            {
                base_launcher_dir.parent().unwrap().to_path_buf()
            } else {
                base_launcher_dir.join("instances").join(&safe_folder_name)
            };

            let mc_dir = instance_root.join("minecraft");
            fs::create_dir_all(&mc_dir)?;

            // Cria instance.cfg caso não exista
            let cfg_path = instance_root.join("instance.cfg");
            if !cfg_path.exists() {
                let cfg_content = format!(
                    "[General]\nAutomaticJava=true\nConfigVersion=1.3\nInstanceType=OneSix\niconKey=steve_legacy\nname={}\n",
                    instance_name
                );
                fs::write(&cfg_path, cfg_content)?;
            }

            // Cria/atualiza mmc-pack.json com a versão exata do Minecraft e do Loader
            let lwjgl_ver = manifest.lwjgl_version.as_deref().unwrap_or("3.3.3");
            let mut components = vec![
                json!({
                    "cachedName": "LWJGL 3",
                    "cachedVersion": lwjgl_ver,
                    "cachedVolatile": true,
                    "dependencyOnly": true,
                    "uid": "org.lwjgl3",
                    "version": lwjgl_ver
                }),
                json!({
                    "cachedName": "Minecraft",
                    "cachedRequires": [{ "suggests": lwjgl_ver, "uid": "org.lwjgl3" }],
                    "cachedVersion": manifest.minecraft_version,
                    "important": true,
                    "uid": "net.minecraft",
                    "version": manifest.minecraft_version
                }),
            ];

            if manifest.loader.name.eq_ignore_ascii_case("fabric") {
                components.push(json!({
                    "cachedName": "Intermediary Mappings",
                    "cachedRequires": [{ "equals": manifest.minecraft_version, "uid": "net.minecraft" }],
                    "cachedVersion": manifest.minecraft_version,
                    "cachedVolatile": true,
                    "dependencyOnly": true,
                    "uid": "net.fabricmc.intermediary",
                    "version": manifest.minecraft_version
                }));
                components.push(json!({
                    "cachedName": "Fabric Loader",
                    "cachedRequires": [{ "uid": "net.fabricmc.intermediary" }],
                    "cachedVersion": manifest.loader.version,
                    "uid": "net.fabricmc.fabric-loader",
                    "version": manifest.loader.version
                }));
            } else {
                components.push(json!({
                    "cachedName": manifest.loader.name,
                    "cachedVersion": manifest.loader.version,
                    "uid": manifest.loader.uid,
                    "version": manifest.loader.version
                }));
            }

            let mmc_pack = json!({
                "components": components,
                "formatVersion": 1
            });

            let mmc_path = instance_root.join("mmc-pack.json");
            fs::write(&mmc_path, serde_json::to_string_pretty(&mmc_pack)?)?;

            Ok(mc_dir)
        }
        "atlauncher" => {
            let folder_slug = safe_folder_name.replace(' ', "");
            let instance_dir = base_launcher_dir.join("instances").join(&folder_slug);
            fs::create_dir_all(&instance_dir)?;

            let instance_json_path = instance_dir.join("instance.json");
            let existing_uuid = if instance_json_path.exists() {
                fs::read_to_string(&instance_json_path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                    .and_then(|v| v.get("uuid").and_then(|u| u.as_str()).map(|s| s.to_string()))
            } else {
                None
            };

            let at_json = json!({
                "uuid": existing_uuid.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                "launcher": {
                    "name": instance_name,
                    "pack": instance_name,
                    "version": manifest.version,
                    "minecraft": manifest.minecraft_version,
                    "loaderVersion": {
                        "type": manifest.loader.name,
                        "version": manifest.loader.version
                    }
                }
            });
            fs::write(&instance_json_path, serde_json::to_string_pretty(&at_json)?)?;

            Ok(instance_dir)
        }
        _ => {
            // Minecraft Launcher Oficial (.minecraft)
            fs::create_dir_all(base_launcher_dir)?;
            let profiles_path = base_launcher_dir.join("launcher_profiles.json");
            let version_id = match manifest.loader.name.to_lowercase().as_str() {
                "fabric" => format!(
                    "fabric-loader-{}-{}",
                    manifest.loader.version, manifest.minecraft_version
                ),
                "quilt" => format!(
                    "quilt-loader-{}-{}",
                    manifest.loader.version, manifest.minecraft_version
                ),
                "neoforge" => format!("neoforge-{}", manifest.loader.version),
                "forge" => format!(
                    "{}-forge-{}",
                    manifest.minecraft_version, manifest.loader.version
                ),
                _ => manifest.minecraft_version.clone(),
            };

            let mut root_json = if profiles_path.exists() {
                let raw = fs::read_to_string(&profiles_path).unwrap_or_default();
                serde_json::from_str::<serde_json::Value>(&raw).unwrap_or_else(|_| json!({}))
            } else {
                json!({})
            };

            if !root_json.get("profiles").map(|p| p.is_object()).unwrap_or(false) {
                root_json["profiles"] = json!({});
            }

            if let Some(profiles_obj) = root_json["profiles"].as_object_mut() {
                profiles_obj.insert(
                    "modpack_sync_launcher".to_string(),
                    json!({
                        "name": instance_name,
                        "type": "custom",
                        "created": chrono::Utc::now().to_rfc3339(),
                        "lastUsed": chrono::Utc::now().to_rfc3339(),
                        "icon": "Crafting_Table",
                        "lastVersionId": version_id,
                        "gameDir": base_launcher_dir.to_string_lossy().to_string()
                    }),
                );
            }

            fs::write(&profiles_path, serde_json::to_string_pretty(&root_json)?)?;
            Ok(base_launcher_dir.to_path_buf())
        }
    }
}

/// Extrai um arquivo `.zip` em memória ou disco para a pasta destino `target_dir`.
pub fn extract_zip_bytes(zip_bytes: &[u8], target_dir: &Path) -> Result<()> {
    let reader = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let Some(enclosed_path) = file.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };

        let outpath = target_dir.join(enclosed_path);
        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

/// Sincroniza e instala o Modpack completo no diretório do usuário:
/// 1. Configura a instância no Launcher selecionado
/// 2. Remove mods/resourcepacks/shaderpacks antigos que saíram do manifesto
/// 3. Baixa em paralelo todos os itens faltantes do Modrinth / CurseForge
/// 4. Aplica `configs.zip` e `custom_overrides.zip`
pub async fn sync_and_install_modpack(
    app: &AppHandle,
    launcher_id: &str,
    base_launcher_path: &str,
    instance_name: &str,
    manifest: &ModpackManifest,
    local_source_fallback: Option<&str>,
) -> Result<String> {
    let base_dir = PathBuf::from(base_launcher_path);
    let mc_dir = configure_launcher_instance(launcher_id, &base_dir, instance_name, manifest)?;

    // Garante que as pastas existam
    for cat in [
        ItemCategory::Mods,
        ItemCategory::Resourcepacks,
        ItemCategory::Shaderpacks,
    ] {
        fs::create_dir_all(mc_dir.join(cat.folder_name()))?;
    }

    // 1. Limpeza de arquivos órfãos (removidos do manifesto oficial)
    let _ = app.emit(
        "download-progress",
        DownloadProgressEvent {
            file_name: "Verificando arquivos antigos...".to_string(),
            category: "sistema".to_string(),
            downloaded_bytes: 0,
            total_bytes: 100,
            completed_files: 0,
            total_files: manifest.items.len(),
            phase: "cleaning_orphans".to_string(),
        },
    );

    for cat in [
        ItemCategory::Mods,
        ItemCategory::Resourcepacks,
        ItemCategory::Shaderpacks,
    ] {
        let folder = mc_dir.join(cat.folder_name());
        let expected_names: HashSet<&str> = manifest
            .items
            .iter()
            .filter(|i| i.category == cat)
            .map(|i| i.file_name.as_str())
            .collect();

        if let Ok(entries) = fs::read_dir(&folder) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file() {
                    let fname = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if (fname.ends_with(".jar") || fname.ends_with(".zip"))
                        && !expected_names.contains(fname.as_str())
                    {
                        let _ = fs::remove_file(&path);
                    }
                }
            }
        }
    }

    // 2. Filtrar itens que precisam ser baixados (não existem localmente ou SHA-1 diverge)
    let mut to_download = Vec::new();
    for item in &manifest.items {
        let target_path = mc_dir
            .join(item.category.folder_name())
            .join(&item.file_name);
        let mut needs_dl = true;
        if target_path.is_file() {
            if let Ok(hashes) = compute_file_hashes(&target_path) {
                if hashes.sha1 == item.sha1 {
                    needs_dl = false;
                }
            }
        }
        if needs_dl {
            to_download.push(item.clone());
        }
    }

    let total_to_download = to_download.len();
    let completed_counter = Arc::new(AtomicUsize::new(0));
    let client = build_http_client()?;

    // 3. Downloads paralelos (até 6 conexões simultâneas)
    let results: Vec<Result<()>> = stream::iter(to_download)
        .map(|item| {
            let client = client.clone();
            let mc_dir = mc_dir.clone();
            let app = app.clone();
            let completed_counter = Arc::clone(&completed_counter);
            let local_fallback = local_source_fallback.map(|s| PathBuf::from(s));

            async move {
                let dest_path = mc_dir
                    .join(item.category.folder_name())
                    .join(&item.file_name);

                if let Some(url) = &item.download_url {
                    let mut resp = client
                        .get(url)
                        .send()
                        .await
                        .with_context(|| format!("Falha ao baixar {}", item.file_name))?;

                    let total_bytes = resp.content_length().unwrap_or(item.file_size);
                    let mut downloaded: u64 = 0;
                    let mut out_file = File::create(&dest_path)?;

                    while let Some(chunk) = resp.chunk().await? {
                        out_file.write_all(&chunk)?;
                        downloaded += chunk.len() as u64;
                        let done_now = completed_counter.load(Ordering::Relaxed);
                        let _ = app.emit(
                            "download-progress",
                            DownloadProgressEvent {
                                file_name: item.file_name.clone(),
                                category: item.category.folder_name().to_string(),
                                downloaded_bytes: downloaded,
                                total_bytes,
                                completed_files: done_now,
                                total_files: total_to_download,
                                phase: "downloading".to_string(),
                            },
                        );
                    }
                } else if item.platform == ModPlatform::CustomOverride {
                    // Se estivermos testando localmente ou antes do overrides.zip, copia da origem se disponível
                    if let Some(src_root) = local_fallback {
                        let src_file = src_root
                            .join(item.category.folder_name())
                            .join(&item.file_name);
                        if src_file.is_file() {
                            fs::copy(&src_file, &dest_path)?;
                        }
                    }
                }

                let finished = completed_counter.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = app.emit(
                    "download-progress",
                    DownloadProgressEvent {
                        file_name: item.file_name.clone(),
                        category: item.category.folder_name().to_string(),
                        downloaded_bytes: item.file_size,
                        total_bytes: item.file_size,
                        completed_files: finished,
                        total_files: total_to_download,
                        phase: "downloading".to_string(),
                    },
                );

                Ok(())
            }
        })
        .buffer_unordered(6)
        .collect()
        .await;

    for res in results {
        res?;
    }

    // 4. Extrair `custom_overrides.zip` e `configs.zip` se as URLs estiverem presentes no manifesto
    if let Some(overrides_url) = &manifest.overrides_download_url {
        if let Ok(resp) = client.get(overrides_url).send().await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    let _ = extract_zip_bytes(&bytes, &mc_dir);
                }
            }
        }
    }

    let _ = app.emit(
        "download-progress",
        DownloadProgressEvent {
            file_name: "Aplicando pasta config/ e configurações...".to_string(),
            category: "config".to_string(),
            downloaded_bytes: 100,
            total_bytes: 100,
            completed_files: total_to_download,
            total_files: total_to_download,
            phase: "extracting_configs".to_string(),
        },
    );

    if let Some(configs_url) = &manifest.configs_download_url {
        if let Ok(resp) = client.get(configs_url).send().await {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    extract_zip_bytes(&bytes, &mc_dir)?;
                }
            }
        }
    } else if let Some(src_root_str) = local_source_fallback {
        // Fallback local: aplica os arquivos de config diretamente da pasta fonte quando testado localmente
        let src_root = PathBuf::from(src_root_str);
        if src_root.exists() && src_root != mc_dir {
            for cfg in &manifest.config_entries {
                let src_p = src_root.join(&cfg.relative_path);
                let dst_p = mc_dir.join(&cfg.relative_path);
                if src_p.is_file() {
                    if let Some(parent) = dst_p.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let _ = fs::copy(&src_p, &dst_p);
                }
            }
        }
    }

    let _ = app.emit(
        "download-progress",
        DownloadProgressEvent {
            file_name: "Modpack sincronizado com sucesso!".to_string(),
            category: "concluido".to_string(),
            downloaded_bytes: 100,
            total_bytes: 100,
            completed_files: total_to_download,
            total_files: total_to_download,
            phase: "done".to_string(),
        },
    );

    Ok(mc_dir.to_string_lossy().to_string())
}

/// Atualiza in-place os mods/resourcepacks/shaderpacks existentes no diretório do usuário
/// sem exigir uma nova Release no GitHub.
pub async fn apply_in_place_updates(
    app: &AppHandle,
    minecraft_dir_str: &str,
    candidates: Vec<InPlaceUpdateCandidate>,
) -> Result<usize> {
    let mc_dir = PathBuf::from(minecraft_dir_str);
    let total = candidates.len();
    if total == 0 {
        return Ok(0);
    }

    let client = build_http_client()?;
    let completed = Arc::new(AtomicUsize::new(0));

    let results: Vec<Result<()>> = stream::iter(candidates)
        .map(|cand| {
            let client = client.clone();
            let mc_dir = mc_dir.clone();
            let app = app.clone();
            let completed = Arc::clone(&completed);

            async move {
                let folder = mc_dir.join(cand.category.folder_name());
                fs::create_dir_all(&folder)?;
                let old_path = folder.join(&cand.current_file_name);
                let new_path = folder.join(&cand.latest_file_name);

                let mut resp = client
                    .get(&cand.latest_download_url)
                    .send()
                    .await
                    .with_context(|| format!("Falha ao baixar atualização {}", cand.latest_file_name))?;

                let total_bytes = resp.content_length().unwrap_or(cand.latest_size);
                let mut downloaded = 0u64;
                let mut out_file = File::create(&new_path)?;

                while let Some(chunk) = resp.chunk().await? {
                    out_file.write_all(&chunk)?;
                    downloaded += chunk.len() as u64;
                    let done = completed.load(Ordering::Relaxed);
                    let _ = app.emit(
                        "download-progress",
                        DownloadProgressEvent {
                            file_name: cand.latest_file_name.clone(),
                            category: cand.category.folder_name().to_string(),
                            downloaded_bytes: downloaded,
                            total_bytes,
                            completed_files: done,
                            total_files: total,
                            phase: "downloading".to_string(),
                        },
                    );
                }

                if old_path != new_path && old_path.exists() {
                    let _ = fs::remove_file(&old_path);
                }

                let finished = completed.fetch_add(1, Ordering::Relaxed) + 1;
                let _ = app.emit(
                    "download-progress",
                    DownloadProgressEvent {
                        file_name: cand.latest_file_name.clone(),
                        category: cand.category.folder_name().to_string(),
                        downloaded_bytes: total_bytes,
                        total_bytes,
                        completed_files: finished,
                        total_files: total,
                        phase: "downloading".to_string(),
                    },
                );

                Ok(())
            }
        })
        .buffer_unordered(4)
        .collect()
        .await;

    for r in results {
        r?;
    }

    let _ = app.emit(
        "download-progress",
        DownloadProgressEvent {
            file_name: "Atualizações in-place concluídas!".to_string(),
            category: "concluido".to_string(),
            downloaded_bytes: 100,
            total_bytes: 100,
            completed_files: total,
            total_files: total,
            phase: "done".to_string(),
        },
    );

    Ok(total)
}

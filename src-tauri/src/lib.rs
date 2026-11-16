pub mod api;
pub mod github;
pub mod hasher;
pub mod installer;
pub mod models;
pub mod scanner;

use crate::api::{check_engine_versions, check_in_place_updates};
use crate::github::{fetch_latest_published_manifest, publish_new_release, PublishReleaseResult};
use crate::installer::{
    apply_in_place_updates, configure_launcher_instance, detect_launcher_paths,
    sync_and_install_modpack,
};
use crate::models::{
    AppSettings, EngineUpdateStatus, InPlaceUpdateCandidate, LauncherPathCandidate, ManifestDiff,
    ModpackManifest,
};
use crate::scanner::{compare_manifests, scan_creator_folder};
use serde::Serialize;
use sha2::{Digest, Sha512};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;

const ADMIN_SALT: &[u8] = b"modpack-sync-launcher-admin-salt-v1:";
const ADMIN_PASSWORD_HASH_HEX: &str =
    "0c9dfffd021dd4fa430a982b43af6ef2f21ec90f5390fa4bfe11aae33b0438133290161b5b03b5ed8a4bd486f1314654bf0550ee0e2ba96673d1737db6126286";

pub fn check_admin_password_hash(candidate: &str) -> bool {
    let mut hasher = Sha512::new();
    hasher.update(ADMIN_SALT);
    hasher.update(candidate.as_bytes());
    let mut digest = hasher.finalize().to_vec();

    for _ in 0..10_000 {
        let mut h = Sha512::new();
        h.update(ADMIN_SALT);
        h.update(&digest);
        digest = h.finalize().to_vec();
    }

    hex::encode(digest) == ADMIN_PASSWORD_HASH_HEX
}

#[tauri::command]
fn verify_admin_password(password: String) -> bool {
    check_admin_password_hash(&password)
}

fn app_config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("modpack-sync-launcher");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn load_settings_from_disk() -> AppSettings {
    let path = app_config_dir().join("settings.json");
    if path.is_file() {
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(mut settings) = serde_json::from_str::<AppSettings>(&raw) {
                if settings.github_repo == "meu-usuario/modpack-amigos"
                    || settings.github_repo.trim().is_empty()
                {
                    settings.github_repo = "odysseexp/modpack-sync-launcher".to_string();
                }
                return settings;
            }
        }
    }
    AppSettings::default()
}

fn save_settings_to_disk(settings: &AppSettings) -> Result<(), String> {
    let path = app_config_dir().join("settings.json");
    let json = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct SystemInitPayload {
    pub detected_os: String,
    pub detected_arch: String,
    pub settings: AppSettings,
    pub launcher_candidates: Vec<LauncherPathCandidate>,
}

#[derive(Debug, Serialize)]
pub struct AdminScanResponse {
    pub manifest: ModpackManifest,
    pub diff: ManifestDiff,
    pub previous_version: Option<String>,
}

#[tauri::command]
fn get_system_info_and_paths(os_override: Option<String>) -> Result<SystemInitPayload, String> {
    let mut settings = load_settings_from_disk();
    let real_os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        _ => "linux",
    };
    let effective_os = os_override
        .as_deref()
        .or(settings.os_theme_override.as_deref())
        .unwrap_or(real_os);

    if effective_os != "linux"
        && (settings.selected_format == "flatpak" || settings.selected_format == "native")
    {
        settings.selected_format = "default".to_string();
    } else if effective_os == "linux" && settings.selected_format == "default" {
        settings.selected_format = "flatpak".to_string();
    }

    let launcher_candidates = detect_launcher_paths(effective_os);

    Ok(SystemInitPayload {
        detected_os: effective_os.to_string(),
        detected_arch: std::env::consts::ARCH.to_string(),
        settings,
        launcher_candidates,
    })
}

#[tauri::command]
fn save_app_settings(settings: AppSettings) -> Result<AppSettings, String> {
    save_settings_to_disk(&settings)?;
    Ok(settings)
}

#[tauri::command]
async fn scan_admin_source(
    source_path: String,
    version_tag: String,
    include_options_txt: bool,
    include_servers_dat: bool,
    github_repo: String,
    github_token: String,
    curseforge_api_key: String,
    admin_password: String,
) -> Result<AdminScanResponse, String> {
    if !check_admin_password_hash(&admin_password) {
        return Err("Acesso negado: senha de administrador inválida.".to_string());
    }

    let manifest = scan_creator_folder(
        &source_path,
        &version_tag,
        include_options_txt,
        include_servers_dat,
        &curseforge_api_key,
    )
    .await
    .map_err(|e| e.to_string())?;

    let cache_dir = app_config_dir().join("staging");
    let previous = fetch_latest_published_manifest(&github_repo, &github_token, &cache_dir)
        .await
        .ok()
        .flatten();

    let diff = compare_manifests(&manifest, previous.as_ref());
    let previous_version = previous.map(|p| p.version);

    Ok(AdminScanResponse {
        manifest,
        diff,
        previous_version,
    })
}

#[tauri::command]
async fn publish_github_release(
    manifest: ModpackManifest,
    source_path: String,
    github_repo: String,
    github_token: String,
    release_notes: String,
    admin_password: String,
) -> Result<PublishReleaseResult, String> {
    if !check_admin_password_hash(&admin_password) {
        return Err("Acesso negado: senha de administrador inválida.".to_string());
    }

    let staging_dir = app_config_dir().join("staging");
    let src_dir = PathBuf::from(source_path);
    publish_new_release(
        manifest,
        &src_dir,
        &staging_dir,
        &github_repo,
        &github_token,
        &release_notes,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn fetch_remote_manifest(
    github_repo: String,
    github_token: String,
    fallback_source_path: String,
    curseforge_api_key: String,
) -> Result<ModpackManifest, String> {
    let cache_dir = app_config_dir().join("staging");
    if let Ok(Some(manifest)) =
        fetch_latest_published_manifest(&github_repo, &github_token, &cache_dir).await
    {
        return Ok(manifest);
    }

    let fallback_path = PathBuf::from(&fallback_source_path);
    if fallback_path.exists() {
        return scan_creator_folder(
            &fallback_source_path,
            "1.0.0",
            true,
            true,
            &curseforge_api_key,
        )
        .await
        .map_err(|e| e.to_string());
    }

    Err("Nenhum manifesto publicado encontrado no repositório ainda.".to_string())
}

#[tauri::command]
async fn sync_install_modpack(
    app: AppHandle,
    launcher_id: String,
    base_launcher_path: String,
    instance_name: String,
    manifest: ModpackManifest,
    local_source_fallback: Option<String>,
) -> Result<String, String> {
    sync_and_install_modpack(
        &app,
        &launcher_id,
        &base_launcher_path,
        &instance_name,
        &manifest,
        local_source_fallback.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_mods_in_place_updates(
    manifest: ModpackManifest,
    curseforge_api_key: String,
) -> Result<Vec<InPlaceUpdateCandidate>, String> {
    check_in_place_updates(
        &manifest.items,
        &manifest.minecraft_version,
        &manifest.loader.name,
        &curseforge_api_key,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_engine_updates_cmd(manifest: ModpackManifest) -> Result<EngineUpdateStatus, String> {
    check_engine_versions(
        &manifest.minecraft_version,
        &manifest.loader.name,
        &manifest.loader.version,
    )
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn apply_engine_version_update(
    launcher_id: String,
    base_launcher_path: String,
    instance_name: String,
    mut manifest: ModpackManifest,
    new_minecraft_version: Option<String>,
    new_loader_version: Option<String>,
    admin_source_path: Option<String>,
) -> Result<ModpackManifest, String> {
    if let Some(mc) = new_minecraft_version {
        manifest.minecraft_version = mc;
    }
    if let Some(ld) = new_loader_version {
        manifest.loader.version = ld;
    }

    let base_dir = PathBuf::from(&base_launcher_path);
    configure_launcher_instance(&launcher_id, &base_dir, &instance_name, &manifest)
        .map_err(|e| e.to_string())?;

    if let Some(src_str) = admin_source_path {
        let src_dir = PathBuf::from(&src_str);
        if src_dir.exists() {
            let _ = configure_launcher_instance("prism", &src_dir, &manifest.name, &manifest);
        }
    }

    Ok(manifest)
}

#[tauri::command]
async fn apply_mods_in_place_updates(
    app: AppHandle,
    launcher_id: String,
    base_launcher_path: String,
    instance_name: String,
    manifest: ModpackManifest,
    candidates: Vec<InPlaceUpdateCandidate>,
) -> Result<usize, String> {
    let base_dir = PathBuf::from(&base_launcher_path);
    let mc_dir = configure_launcher_instance(&launcher_id, &base_dir, &instance_name, &manifest)
        .map_err(|e| e.to_string())?;

    apply_in_place_updates(&app, &mc_dir.to_string_lossy(), candidates)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_system_info_and_paths,
            save_app_settings,
            verify_admin_password,
            scan_admin_source,
            publish_github_release,
            fetch_remote_manifest,
            sync_install_modpack,
            check_mods_in_place_updates,
            check_engine_updates_cmd,
            apply_engine_version_update,
            apply_mods_in_place_updates,
        ])
        .run(tauri::generate_context!())
        .expect("Erro ao executar Modpack Sync Launcher");
}

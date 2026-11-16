use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemCategory {
    Mods,
    Resourcepacks,
    Shaderpacks,
}

impl ItemCategory {
    pub fn folder_name(&self) -> &'static str {
        match self {
            ItemCategory::Mods => "mods",
            ItemCategory::Resourcepacks => "resourcepacks",
            ItemCategory::Shaderpacks => "shaderpacks",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModPlatform {
    Modrinth,
    Curseforge,
    CustomOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackItem {
    pub file_name: String,
    pub display_name: String,
    pub category: ItemCategory,
    pub platform: ModPlatform,
    pub project_id: Option<String>,
    pub file_id: Option<String>,
    pub download_url: Option<String>,
    pub sha1: String,
    pub sha512: String,
    pub murmur2: u32,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModLoaderInfo {
    pub name: String, // "fabric", "forge", "neoforge", "quilt"
    pub version: String,
    pub uid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigFileEntry {
    pub relative_path: String,
    pub sha1: String,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackManifest {
    pub name: String,
    pub version: String,
    pub published_at: String,
    pub minecraft_version: String,
    pub loader: ModLoaderInfo,
    pub lwjgl_version: Option<String>,
    pub include_options_txt: bool,
    pub include_servers_dat: bool,
    pub configs_archive_sha1: String,
    pub configs_download_url: Option<String>,
    pub overrides_archive_sha1: Option<String>,
    pub overrides_download_url: Option<String>,
    pub items: Vec<PackItem>,
    pub config_entries: Vec<ConfigFileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestDiff {
    pub has_structural_changes: bool,
    pub added_items: Vec<PackItem>,
    pub removed_items: Vec<PackItem>,
    pub version_changed_items: Vec<VersionChangeEntry>,
    pub config_changed_files: Vec<String>,
    pub minecraft_version_change: Option<(String, String)>,
    pub loader_version_change: Option<(String, String)>,
    pub requires_github_release: bool,
    pub summary_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineUpdateStatus {
    pub current_minecraft: String,
    pub latest_minecraft: String,
    pub minecraft_update_available: bool,
    pub current_loader_name: String,
    pub current_loader_version: String,
    pub latest_loader_version: String,
    pub loader_update_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionChangeEntry {
    pub display_name: String,
    pub category: ItemCategory,
    pub old_file_name: String,
    pub new_file_name: String,
    pub platform: ModPlatform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InPlaceUpdateCandidate {
    pub current_file_name: String,
    pub display_name: String,
    pub category: ItemCategory,
    pub platform: ModPlatform,
    pub project_id: String,
    pub current_file_id: String,
    pub latest_file_id: String,
    pub latest_file_name: String,
    pub latest_download_url: String,
    pub latest_sha1: Option<String>,
    pub latest_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadProgressEvent {
    pub file_name: String,
    pub category: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub completed_files: usize,
    pub total_files: usize,
    pub phase: String, // "downloading", "extracting_configs", "cleaning_orphans", "configuring_instance", "done"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherPathCandidate {
    pub launcher_id: String, // "prism", "atlauncher", "official"
    pub format_id: String,   // "default", "flatpak", "native", "custom"
    pub label: String,
    pub resolved_path: String,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub selected_launcher: String,
    pub selected_format: String,
    pub custom_launcher_path: String,
    pub target_instance_name: String,
    pub linux_theme_style: String, // "gtk" | "kde"
    pub os_theme_override: Option<String>, // None = auto ("windows", "linux", "macos")
    pub admin_source_path: String,
    pub github_repo: String, // e.g., "odysseexp/modpack-sync-launcher"
    pub github_token: String,
    pub curseforge_api_key: String,
    pub include_options_txt: bool,
    pub include_servers_dat: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        let default_format = if cfg!(target_os = "linux") {
            "flatpak"
        } else {
            "default"
        };
        Self {
            selected_launcher: "prism".to_string(),
            selected_format: default_format.to_string(),
            custom_launcher_path: String::new(),
            target_instance_name: "Servidor Amigos".to_string(),
            linux_theme_style: "gtk".to_string(),
            os_theme_override: None,
            admin_source_path: "/home/deck/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances/Vanilla/minecraft".to_string(),
            github_repo: "odysseexp/modpack-sync-launcher".to_string(),
            github_token: String::new(),
            curseforge_api_key: "$2a$10$4N9GcONHeSE4m41B.GbXgOlJsaCsYcOGQeZb.xakUtqhYZ2Je2sjm".to_string(),
            include_options_txt: true,
            include_servers_dat: true,
        }
    }
}

export type ItemCategory = "mods" | "resourcepacks" | "shaderpacks";
export type ModPlatform = "modrinth" | "curseforge" | "custom_override";

export interface PackItem {
  file_name: string;
  display_name: string;
  category: ItemCategory;
  platform: ModPlatform;
  project_id: string | null;
  file_id: string | null;
  download_url: string | null;
  sha1: string;
  sha512: string;
  murmur2: number;
  file_size: number;
}

export interface ModLoaderInfo {
  name: string;
  version: string;
  uid: string;
}

export interface ConfigFileEntry {
  relative_path: string;
  sha1: string;
  file_size: number;
}

export interface ModpackManifest {
  name: string;
  version: string;
  published_at: string;
  minecraft_version: string;
  loader: ModLoaderInfo;
  lwjgl_version: string | null;
  include_options_txt: boolean;
  include_servers_dat: boolean;
  configs_archive_sha1: string;
  configs_download_url: string | null;
  overrides_archive_sha1: string | null;
  overrides_download_url: string | null;
  items: PackItem[];
  config_entries: ConfigFileEntry[];
}

export interface VersionChangeEntry {
  display_name: string;
  category: ItemCategory;
  old_file_name: string;
  new_file_name: string;
  platform: ModPlatform;
}

export interface ManifestDiff {
  has_structural_changes: boolean;
  added_items: PackItem[];
  removed_items: PackItem[];
  version_changed_items: VersionChangeEntry[];
  config_changed_files: string[];
  minecraft_version_change: [string, string] | null;
  loader_version_change: [string, string] | null;
  requires_github_release: boolean;
  summary_message: string;
}

export interface EngineUpdateStatus {
  current_minecraft: string;
  latest_minecraft: string;
  minecraft_update_available: boolean;
  current_loader_name: string;
  current_loader_version: string;
  latest_loader_version: string;
  loader_update_available: boolean;
}

export interface InPlaceUpdateCandidate {
  current_file_name: string;
  display_name: string;
  category: ItemCategory;
  platform: ModPlatform;
  project_id: string;
  current_file_id: string;
  latest_file_id: string;
  latest_file_name: string;
  latest_download_url: string;
  latest_sha1: string | null;
  latest_size: number;
}

export interface DownloadProgressEvent {
  file_name: string;
  category: string;
  downloaded_bytes: number;
  total_bytes: number;
  completed_files: number;
  total_files: number;
  phase:
    | "downloading"
    | "extracting_configs"
    | "cleaning_orphans"
    | "configuring_instance"
    | "done";
}

export interface LauncherPathCandidate {
  launcher_id: "prism" | "atlauncher" | "official";
  format_id: "default" | "flatpak" | "native" | "custom";
  label: string;
  resolved_path: string;
  exists: boolean;
}

export interface AppSettings {
  selected_launcher: "prism" | "atlauncher" | "official";
  selected_format: "default" | "flatpak" | "native" | "custom";
  custom_launcher_path: string;
  target_instance_name: string;
  linux_theme_style: "gtk" | "kde";
  os_theme_override: string | null;
  admin_source_path: string;
  github_repo: string;
  github_token: string;
  curseforge_api_key: string;
  include_options_txt: boolean;
  include_servers_dat: boolean;
}

export interface SystemInitPayload {
  detected_os: "windows" | "linux" | "macos";
  detected_arch: string;
  settings: AppSettings;
  launcher_candidates: LauncherPathCandidate[];
}

export interface AdminScanResponse {
  manifest: ModpackManifest;
  diff: ManifestDiff;
  previous_version: string | null;
}

export interface PublishReleaseResult {
  version_tag: string;
  release_url: string;
  manifest_path: string;
  configs_zip_path: string;
  uploaded_to_github: boolean;
  message: string;
}

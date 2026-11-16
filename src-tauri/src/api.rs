use crate::models::{EngineUpdateStatus, InPlaceUpdateCandidate, ModPlatform, PackItem};
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MODRINTH_API_BASE: &str = "https://api.modrinth.com/v2";
const CURSEFORGE_API_BASE: &str = "https://api.curseforge.com/v1";
const FABRIC_META_API_BASE: &str = "https://meta.fabricmc.net/v2";
pub const DEFAULT_CF_API_KEY: &str =
    "$2a$10$4N9GcONHeSE4m41B.GbXgOlJsaCsYcOGQeZb.xakUtqhYZ2Je2sjm";

#[derive(Debug, Deserialize)]
struct FabricGameVersion {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderVersionEntry {
    version: String,
    stable: bool,
}

#[derive(Debug, Deserialize)]
struct FabricLoaderForGameEntry {
    loader: FabricLoaderVersionEntry,
}

#[derive(Debug, Serialize)]
struct ModrinthHashLookupRequest {
    hashes: Vec<String>,
    algorithm: String,
}

#[derive(Debug, Serialize)]
struct ModrinthUpdateRequest {
    hashes: Vec<String>,
    algorithm: String,
    loaders: Vec<String>,
    game_versions: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModrinthVersion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub files: Vec<ModrinthFile>,
}

#[derive(Debug, Deserialize)]
pub struct ModrinthFile {
    pub hashes: ModrinthFileHashes,
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
pub struct ModrinthFileHashes {
    pub sha512: Option<String>,
    pub sha1: Option<String>,
}

#[derive(Debug, Serialize)]
struct CurseForgeFingerprintRequest {
    fingerprints: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct CurseForgeFingerprintResponse {
    data: CurseForgeFingerprintData,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeFingerprintData {
    exact_matches: Vec<CurseForgeMatch>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CurseForgeMatch {
    id: u64, // modId / project_id
    file: CurseForgeFile,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CurseForgeFile {
    pub id: u64,
    pub mod_id: u64,
    pub display_name: String,
    pub file_name: String,
    pub file_length: u64,
    pub download_url: Option<String>,
    pub file_fingerprint: u32,
    pub hashes: Vec<CurseForgeFileHash>,
    pub game_versions: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CurseForgeFileHash {
    pub value: String,
    pub algo: u8, // 1 = sha1, 2 = md5
}

#[derive(Debug, Deserialize)]
struct CurseForgeFilesListResponse {
    data: Vec<CurseForgeFile>,
}

pub fn build_http_client() -> Result<Client> {
    let client = Client::builder()
        .user_agent("ModpackSyncLauncher/1.0.0 (github.com/odysseexp/modpack-sync-launcher)")
        .build()?;
    Ok(client)
}

/// Consulta a API oficial do Fabric Meta (`meta.fabricmc.net`) para detectar se há
/// uma nova versão estável do Minecraft ou do Fabric Loader disponível.
pub async fn check_engine_versions(
    current_mc: &str,
    current_loader_name: &str,
    current_loader_ver: &str,
) -> Result<EngineUpdateStatus> {
    let client = build_http_client()?;

    let mut latest_mc = current_mc.to_string();
    let mut latest_loader = current_loader_ver.to_string();

    // 1. Consulta versões estáveis do Minecraft suportadas pelo Fabric
    if let Ok(resp) = client
        .get(format!("{}/versions/game", FABRIC_META_API_BASE))
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(games) = resp.json::<Vec<FabricGameVersion>>().await {
                if let Some(first_stable) = games.iter().find(|g| g.stable) {
                    latest_mc = first_stable.version.clone();
                }
            }
        }
    }

    // 2. Consulta a versão mais recente do Fabric Loader para a versão atual do Minecraft
    if current_loader_name.eq_ignore_ascii_case("fabric") {
        let loader_for_mc_url =
            format!("{}/versions/loader/{}", FABRIC_META_API_BASE, current_mc);
        let mut resolved = false;

        if let Ok(resp) = client.get(&loader_for_mc_url).send().await {
            if resp.status().is_success() {
                if let Ok(entries) = resp.json::<Vec<FabricLoaderForGameEntry>>().await {
                    if let Some(stable_entry) = entries
                        .iter()
                        .find(|e| e.loader.stable)
                        .or_else(|| entries.first())
                    {
                        latest_loader = stable_entry.loader.version.clone();
                        resolved = true;
                    }
                }
            }
        }

        if !resolved {
            if let Ok(resp) = client
                .get(format!("{}/versions/loader", FABRIC_META_API_BASE))
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(loaders) = resp.json::<Vec<FabricLoaderVersionEntry>>().await {
                        if let Some(first_stable) =
                            loaders.iter().find(|l| l.stable).or_else(|| loaders.first())
                        {
                            latest_loader = first_stable.version.clone();
                        }
                    }
                }
            }
        }
    }

    let minecraft_update_available = latest_mc != current_mc;
    let loader_update_available = latest_loader != current_loader_ver;

    Ok(EngineUpdateStatus {
        current_minecraft: current_mc.to_string(),
        latest_minecraft: latest_mc,
        minecraft_update_available,
        current_loader_name: current_loader_name.to_string(),
        current_loader_version: current_loader_ver.to_string(),
        latest_loader_version: latest_loader,
        loader_update_available,
    })
}

/// Constrói URL de fallback na CDN oficial do CurseForge caso a API retorne downloadUrl = null
pub fn resolve_curseforge_download_url(
    file_id: u64,
    file_name: &str,
    api_url: Option<String>,
) -> String {
    if let Some(url) = api_url {
        if !url.trim().is_empty() {
            return url;
        }
    }
    let part1 = file_id / 1000;
    let part2 = file_id % 1000;
    let encoded_name = file_name.replace(' ', "%20");
    format!(
        "https://edge.forgecdn.net/files/{}/{}/{}",
        part1, part2, encoded_name
    )
}

/// Resolve uma lista de `PackItem` consultando em lote primeiro o Modrinth (por SHA-1)
/// e depois o CurseForge (por Murmur2 fingerprint) para os itens não encontrados no Modrinth.
/// Qualquer arquivo não encontrado em ambos permanece como `ModPlatform::CustomOverride`.
pub async fn resolve_items_metadata(
    items: &mut [PackItem],
    curseforge_api_key: &str,
) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }

    let client = build_http_client()?;

    // 1. Consulta em lote no Modrinth API (SHA-1)
    let sha1_list: Vec<String> = items.iter().map(|i| i.sha1.clone()).collect();
    let mr_req = ModrinthHashLookupRequest {
        hashes: sha1_list,
        algorithm: "sha1".to_string(),
    };

    if let Ok(resp) = client
        .post(format!("{}/version_files", MODRINTH_API_BASE))
        .json(&mr_req)
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(map) = resp.json::<HashMap<String, ModrinthVersion>>().await {
                for item in items.iter_mut() {
                    if let Some(ver) = map.get(&item.sha1) {
                        let matched_file = ver
                            .files
                            .iter()
                            .find(|f| f.hashes.sha1.as_deref() == Some(&item.sha1))
                            .or_else(|| ver.files.iter().find(|f| f.primary))
                            .or_else(|| ver.files.first());

                        if let Some(file) = matched_file {
                            item.platform = ModPlatform::Modrinth;
                            item.project_id = Some(ver.project_id.clone());
                            item.file_id = Some(ver.id.clone());
                            item.download_url = Some(file.url.clone());
                            if !ver.name.is_empty() {
                                item.display_name = ver.name.clone();
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Para os itens ainda não resolvidos, consulta o CurseForge via Murmur2 Fingerprint
    let unresolved_fingerprints: Vec<u32> = items
        .iter()
        .filter(|i| i.platform == ModPlatform::CustomOverride)
        .map(|i| i.murmur2)
        .collect();

    if !unresolved_fingerprints.is_empty() {
        let api_key = if curseforge_api_key.trim().is_empty() {
            DEFAULT_CF_API_KEY
        } else {
            curseforge_api_key.trim()
        };

        let cf_req = CurseForgeFingerprintRequest {
            fingerprints: unresolved_fingerprints,
        };

        if let Ok(resp) = client
            .post(format!("{}/fingerprints/432", CURSEFORGE_API_BASE))
            .header("x-api-key", api_key)
            .header("Accept", "application/json")
            .json(&cf_req)
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(cf_resp) = resp.json::<CurseForgeFingerprintResponse>().await {
                    let mut by_fp: HashMap<u32, CurseForgeMatch> = HashMap::new();
                    for m in cf_resp.data.exact_matches {
                        by_fp.insert(m.file.file_fingerprint, m);
                    }

                    for item in items.iter_mut() {
                        if item.platform == ModPlatform::CustomOverride {
                            if let Some(m) = by_fp.get(&item.murmur2) {
                                item.platform = ModPlatform::Curseforge;
                                item.project_id = Some(m.id.to_string());
                                item.file_id = Some(m.file.id.to_string());
                                item.download_url = Some(resolve_curseforge_download_url(
                                    m.file.id,
                                    &m.file.file_name,
                                    m.file.download_url.clone(),
                                ));
                                if !m.file.display_name.is_empty() {
                                    item.display_name = m.file.display_name.clone();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Consulta as APIs do Modrinth e CurseForge para verificar se existem versões mais recentes
/// dos mods/resourcepacks/shaderpacks atuais para a mesma versão do Minecraft e Mod Loader
/// (Atualização In-Place sem exigir nova Release no GitHub).
pub async fn check_in_place_updates(
    items: &[PackItem],
    minecraft_version: &str,
    loader_name: &str,
    curseforge_api_key: &str,
) -> Result<Vec<InPlaceUpdateCandidate>> {
    let client = build_http_client()?;
    let mut updates = Vec::new();

    // 1. Verificar atualizações no Modrinth via /v2/version_files/update
    let modrinth_items: Vec<&PackItem> = items
        .iter()
        .filter(|i| i.platform == ModPlatform::Modrinth)
        .collect();

    if !modrinth_items.is_empty() {
        let hashes: Vec<String> = modrinth_items.iter().map(|i| i.sha1.clone()).collect();
        let req = ModrinthUpdateRequest {
            hashes,
            algorithm: "sha1".to_string(),
            loaders: vec![loader_name.to_lowercase()],
            game_versions: vec![minecraft_version.to_string()],
        };

        if let Ok(resp) = client
            .post(format!("{}/version_files/update", MODRINTH_API_BASE))
            .json(&req)
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(latest_map) = resp.json::<HashMap<String, ModrinthVersion>>().await {
                    for item in modrinth_items {
                        if let Some(latest_ver) = latest_map.get(&item.sha1) {
                            let current_version_id = item.file_id.as_deref().unwrap_or("");
                            if latest_ver.id != current_version_id {
                                if let Some(primary_file) = latest_ver
                                    .files
                                    .iter()
                                    .find(|f| f.primary)
                                    .or_else(|| latest_ver.files.first())
                                {
                                    if primary_file.filename != item.file_name {
                                        updates.push(InPlaceUpdateCandidate {
                                            current_file_name: item.file_name.clone(),
                                            display_name: item.display_name.clone(),
                                            category: item.category.clone(),
                                            platform: ModPlatform::Modrinth,
                                            project_id: latest_ver.project_id.clone(),
                                            current_file_id: current_version_id.to_string(),
                                            latest_file_id: latest_ver.id.clone(),
                                            latest_file_name: primary_file.filename.clone(),
                                            latest_download_url: primary_file.url.clone(),
                                            latest_sha1: primary_file.hashes.sha1.clone(),
                                            latest_size: primary_file.size,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Verificar atualizações no CurseForge por projeto
    let cf_items: Vec<&PackItem> = items
        .iter()
        .filter(|i| i.platform == ModPlatform::Curseforge)
        .collect();

    if !cf_items.is_empty() {
        let api_key = if curseforge_api_key.trim().is_empty() {
            DEFAULT_CF_API_KEY
        } else {
            curseforge_api_key.trim()
        };

        let cf_loader_id = match loader_name.to_lowercase().as_str() {
            "forge" => Some(1),
            "fabric" => Some(4),
            "quilt" => Some(5),
            "neoforge" => Some(6),
            _ => None,
        };

        for item in cf_items {
            let Some(project_id) = &item.project_id else {
                continue;
            };
            let current_file_id = item.file_id.as_deref().unwrap_or("0");

            let mut url = format!(
                "{}/mods/{}/files?gameVersion={}&pageSize=5",
                CURSEFORGE_API_BASE, project_id, minecraft_version
            );
            if item.category == crate::models::ItemCategory::Mods {
                if let Some(lid) = cf_loader_id {
                    url.push_str(&format!("&modLoaderType={}", lid));
                }
            }

            if let Ok(resp) = client
                .get(&url)
                .header("x-api-key", api_key)
                .header("Accept", "application/json")
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(list_resp) = resp.json::<CurseForgeFilesListResponse>().await {
                        if let Some(latest_file) = list_resp.data.first() {
                            let latest_id_str = latest_file.id.to_string();
                            let current_id_num: u64 = current_file_id.parse().unwrap_or(0);
                            if latest_file.id > current_id_num
                                && latest_file.file_name != item.file_name
                            {
                                let sha1 = latest_file
                                    .hashes
                                    .iter()
                                    .find(|h| h.algo == 1)
                                    .map(|h| h.value.clone());

                                updates.push(InPlaceUpdateCandidate {
                                    current_file_name: item.file_name.clone(),
                                    display_name: item.display_name.clone(),
                                    category: item.category.clone(),
                                    platform: ModPlatform::Curseforge,
                                    project_id: project_id.clone(),
                                    current_file_id: current_file_id.to_string(),
                                    latest_file_id: latest_id_str,
                                    latest_file_name: latest_file.file_name.clone(),
                                    latest_download_url: resolve_curseforge_download_url(
                                        latest_file.id,
                                        &latest_file.file_name,
                                        latest_file.download_url.clone(),
                                    ),
                                    latest_sha1: sha1,
                                    latest_size: latest_file.file_length,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(updates)
}

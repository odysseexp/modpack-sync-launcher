use crate::api::resolve_items_metadata;
use crate::hasher::compute_file_hashes;
use crate::models::{
    ConfigFileEntry, ItemCategory, ManifestDiff, ModLoaderInfo, ModPlatform, ModpackManifest,
    PackItem, VersionChangeEntry,
};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::Deserialize;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

#[derive(Debug, Deserialize)]
struct MmcPackJson {
    components: Vec<MmcComponent>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MmcComponent {
    uid: String,
    version: Option<String>,
    cached_version: Option<String>,
}

/// Extrai metadados de versão do Minecraft, Mod Loader e nome da instância a partir de
/// `mmc-pack.json` e `instance.cfg` na pasta pai da instância PrismLauncher.
pub fn parse_prism_instance_metadata(
    minecraft_dir: &Path,
) -> (String, String, ModLoaderInfo, Option<String>) {
    let mut instance_name = "Modpack Sync".to_string();
    let mut mc_version = "1.21.1".to_string();
    let mut loader = ModLoaderInfo {
        name: "fabric".to_string(),
        version: "0.16.9".to_string(),
        uid: "net.fabricmc.fabric-loader".to_string(),
    };
    let mut lwjgl_version = None;

    if let Some(parent) = minecraft_dir.parent() {
        let cfg_path = parent.join("instance.cfg");
        if let Ok(cfg_text) = fs::read_to_string(&cfg_path) {
            for line in cfg_text.lines() {
                if let Some(val) = line.strip_prefix("name=") {
                    if !val.trim().is_empty() {
                        instance_name = val.trim().to_string();
                    }
                }
            }
        }

        let mmc_path = parent.join("mmc-pack.json");
        if let Ok(mmc_text) = fs::read_to_string(&mmc_path) {
            if let Ok(mmc) = serde_json::from_str::<MmcPackJson>(&mmc_text) {
                for comp in mmc.components {
                    let ver = comp
                        .version
                        .or(comp.cached_version)
                        .unwrap_or_default();
                    match comp.uid.as_str() {
                        "net.minecraft" => {
                            if !ver.is_empty() {
                                mc_version = ver;
                            }
                        }
                        "org.lwjgl3" | "org.lwjgl" => {
                            if !ver.is_empty() {
                                lwjgl_version = Some(ver);
                            }
                        }
                        "net.fabricmc.fabric-loader" => {
                            loader = ModLoaderInfo {
                                name: "fabric".to_string(),
                                version: ver,
                                uid: comp.uid,
                            };
                        }
                        "net.minecraftforge" => {
                            loader = ModLoaderInfo {
                                name: "forge".to_string(),
                                version: ver,
                                uid: comp.uid,
                            };
                        }
                        "net.neoforged" => {
                            loader = ModLoaderInfo {
                                name: "neoforge".to_string(),
                                version: ver,
                                uid: comp.uid,
                            };
                        }
                        "org.quiltmc.quilt-loader" => {
                            loader = ModLoaderInfo {
                                name: "quilt".to_string(),
                                version: ver,
                                uid: comp.uid,
                            };
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    (instance_name, mc_version, loader, lwjgl_version)
}

/// Escaneia uma subpasta (`mods/`, `resourcepacks/` ou `shaderpacks/`) e calcula os hashes
/// SHA-1, SHA-512 e Murmur2 de cada arquivo válido (.jar ou .zip).
fn scan_category_folder(base_dir: &Path, category: ItemCategory) -> Result<Vec<PackItem>> {
    let folder = base_dir.join(category.folder_name());
    let mut items = Vec::new();
    if !folder.exists() {
        return Ok(items);
    }

    let mut entries: Vec<PathBuf> = fs::read_dir(&folder)?
        .filter_map(|e| e.ok().map(|entry| entry.path()))
        .filter(|p| p.is_file())
        .collect();

    entries.sort();

    for path in entries {
        let Some(file_name_os) = path.file_name() else {
            continue;
        };
        let file_name = file_name_os.to_string_lossy().to_string();

        // Ignora arquivos ocultos ou desativados
        if file_name.starts_with('.') || file_name.ends_with(".disabled") {
            continue;
        }

        let is_valid_ext = file_name.ends_with(".jar") || file_name.ends_with(".zip");
        if !is_valid_ext {
            continue;
        }

        let hashes = compute_file_hashes(&path)?;
        let clean_title = file_name
            .trim_end_matches(".jar")
            .trim_end_matches(".zip")
            .to_string();

        items.push(PackItem {
            file_name,
            display_name: clean_title,
            category: category.clone(),
            platform: ModPlatform::CustomOverride,
            project_id: None,
            file_id: None,
            download_url: None,
            sha1: hashes.sha1,
            sha512: hashes.sha512,
            murmur2: hashes.murmur2,
            file_size: hashes.file_size,
        });
    }

    Ok(items)
}

/// Escaneia a pasta `config/` (e opcionalmente `options.txt` e `servers.dat`) para detectar
/// qualquer alteração de configuração e gerar um hash determinístico do conjunto de configs.
pub fn scan_config_entries(
    minecraft_dir: &Path,
    include_options_txt: bool,
    include_servers_dat: bool,
) -> Result<(Vec<ConfigFileEntry>, String)> {
    let mut entries = Vec::new();
    let config_dir = minecraft_dir.join("config");

    if config_dir.exists() {
        for entry in WalkDir::new(&config_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Ok(rel) = path.strip_prefix(minecraft_dir) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    if let Ok(bytes) = fs::read(path) {
                        let mut hasher = Sha1::new();
                        hasher.update(&bytes);
                        let sha1 = hex::encode(hasher.finalize());
                        entries.push(ConfigFileEntry {
                            relative_path: rel_str,
                            sha1,
                            file_size: bytes.len() as u64,
                        });
                    }
                }
            }
        }
    }

    if include_options_txt {
        let opt_path = minecraft_dir.join("options.txt");
        if opt_path.is_file() {
            if let Ok(bytes) = fs::read(&opt_path) {
                let mut hasher = Sha1::new();
                hasher.update(&bytes);
                entries.push(ConfigFileEntry {
                    relative_path: "options.txt".to_string(),
                    sha1: hex::encode(hasher.finalize()),
                    file_size: bytes.len() as u64,
                });
            }
        }
    }

    if include_servers_dat {
        let srv_path = minecraft_dir.join("servers.dat");
        if srv_path.is_file() {
            if let Ok(bytes) = fs::read(&srv_path) {
                let mut hasher = Sha1::new();
                hasher.update(&bytes);
                entries.push(ConfigFileEntry {
                    relative_path: "servers.dat".to_string(),
                    sha1: hex::encode(hasher.finalize()),
                    file_size: bytes.len() as u64,
                });
            }
        }
    }

    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

    let mut combined_hasher = Sha1::new();
    for item in &entries {
        combined_hasher.update(item.relative_path.as_bytes());
        combined_hasher.update(b":");
        combined_hasher.update(item.sha1.as_bytes());
        combined_hasher.update(b"\n");
    }
    let combined_sha1 = hex::encode(combined_hasher.finalize());

    Ok((entries, combined_sha1))
}

/// Realiza o escaneamento completo da pasta fonte do Criador, resolvendo hashes no
/// Modrinth e no CurseForge e montando o `ModpackManifest`.
pub async fn scan_creator_folder(
    source_dir_str: &str,
    version_tag: &str,
    include_options_txt: bool,
    include_servers_dat: bool,
    curseforge_api_key: &str,
) -> Result<ModpackManifest> {
    let minecraft_dir = PathBuf::from(source_dir_str);
    if !minecraft_dir.exists() {
        anyhow::bail!(
            "A pasta de origem não existe: {}",
            minecraft_dir.display()
        );
    }

    let (instance_name, mc_version, loader, lwjgl_version) =
        parse_prism_instance_metadata(&minecraft_dir);

    let mut all_items = Vec::new();
    all_items.extend(scan_category_folder(&minecraft_dir, ItemCategory::Mods)?);
    all_items.extend(scan_category_folder(
        &minecraft_dir,
        ItemCategory::Resourcepacks,
    )?);
    all_items.extend(scan_category_folder(
        &minecraft_dir,
        ItemCategory::Shaderpacks,
    )?);

    // Resolve junto às APIs do Modrinth e CurseForge
    resolve_items_metadata(&mut all_items, curseforge_api_key).await?;

    let (config_entries, configs_archive_sha1) = scan_config_entries(
        &minecraft_dir,
        include_options_txt,
        include_servers_dat,
    )?;

    let has_overrides = all_items
        .iter()
        .any(|i| i.platform == ModPlatform::CustomOverride);

    let overrides_archive_sha1 = if has_overrides {
        let mut h = Sha1::new();
        for o in all_items
            .iter()
            .filter(|i| i.platform == ModPlatform::CustomOverride)
        {
            h.update(o.file_name.as_bytes());
            h.update(o.sha1.as_bytes());
        }
        Some(hex::encode(h.finalize()))
    } else {
        None
    };

    Ok(ModpackManifest {
        name: instance_name,
        version: version_tag.to_string(),
        published_at: Utc::now().to_rfc3339(),
        minecraft_version: mc_version,
        loader,
        lwjgl_version,
        include_options_txt,
        include_servers_dat,
        configs_archive_sha1,
        configs_download_url: None,
        overrides_archive_sha1,
        overrides_download_url: None,
        items: all_items,
        config_entries,
    })
}

/// Chave lógica que identifica um projeto único (independente da versão do arquivo)
fn item_identity_key(item: &PackItem) -> String {
    if let Some(pid) = &item.project_id {
        format!("{:?}:{:?}:{}", item.category, item.platform, pid)
    } else {
        // Para custom_override sem project_id, usa o nome do arquivo
        format!("{:?}:custom:{}", item.category, item.file_name)
    }
}

/// Compara o manifesto local recém-escaneado com o último `modpack-manifest.json` publicado.
/// Regra Crítica:
/// - Adição/remoção de itens ou mudança em `config/` -> `requires_github_release = true`
/// - Apenas atualização de versão dos mesmos projetos -> `requires_github_release = false`
pub fn compare_manifests(
    current: &ModpackManifest,
    previous: Option<&ModpackManifest>,
) -> ManifestDiff {
    let Some(prev) = previous else {
        return ManifestDiff {
            has_structural_changes: true,
            added_items: current.items.clone(),
            removed_items: Vec::new(),
            version_changed_items: Vec::new(),
            config_changed_files: current
                .config_entries
                .iter()
                .map(|c| c.relative_path.clone())
                .collect(),
            minecraft_version_change: None,
            loader_version_change: None,
            requires_github_release: true,
            summary_message: "Primeira publicação do manifesto detectada. Pronta para criar Release no GitHub.".to_string(),
        };
    };

    let mut prev_by_key: HashMap<String, &PackItem> = HashMap::new();
    for item in &prev.items {
        prev_by_key.insert(item_identity_key(item), item);
    }

    let mut curr_by_key: HashMap<String, &PackItem> = HashMap::new();
    for item in &current.items {
        curr_by_key.insert(item_identity_key(item), item);
    }

    let mut added_items = Vec::new();
    let mut removed_items = Vec::new();
    let mut version_changed_items = Vec::new();

    for (key, curr_item) in &curr_by_key {
        if let Some(prev_item) = prev_by_key.get(key) {
            if curr_item.sha1 != prev_item.sha1 {
                version_changed_items.push(VersionChangeEntry {
                    display_name: curr_item.display_name.clone(),
                    category: curr_item.category.clone(),
                    old_file_name: prev_item.file_name.clone(),
                    new_file_name: curr_item.file_name.clone(),
                    platform: curr_item.platform.clone(),
                });
            }
        } else {
            added_items.push((*curr_item).clone());
        }
    }

    for (key, prev_item) in &prev_by_key {
        if !curr_by_key.contains_key(key) {
            removed_items.push((*prev_item).clone());
        }
    }

    // Comparar arquivos de config
    let mut prev_configs: HashMap<&str, &str> = HashMap::new();
    for c in &prev.config_entries {
        prev_configs.insert(&c.relative_path, &c.sha1);
    }

    let mut curr_configs: HashMap<&str, &str> = HashMap::new();
    for c in &current.config_entries {
        curr_configs.insert(&c.relative_path, &c.sha1);
    }

    let mut config_changed_files = Vec::new();
    let all_config_paths: HashSet<&str> = prev_configs
        .keys()
        .chain(curr_configs.keys())
        .copied()
        .collect();

    for path in all_config_paths {
        match (prev_configs.get(path), curr_configs.get(path)) {
            (Some(old_sha), Some(new_sha)) if old_sha != new_sha => {
                config_changed_files.push(format!("Modificado: {}", path));
            }
            (None, Some(_)) => {
                config_changed_files.push(format!("Adicionado: {}", path));
            }
            (Some(_), None) => {
                config_changed_files.push(format!("Removido: {}", path));
            }
            _ => {}
        }
    }
    config_changed_files.sort();

    let minecraft_version_change = if current.minecraft_version != prev.minecraft_version {
        Some((
            prev.minecraft_version.clone(),
            current.minecraft_version.clone(),
        ))
    } else {
        None
    };

    let loader_version_change = if current.loader.name != prev.loader.name
        || current.loader.version != prev.loader.version
    {
        Some((
            format!("{} {}", prev.loader.name, prev.loader.version),
            format!("{} {}", current.loader.name, current.loader.version),
        ))
    } else {
        None
    };

    let loader_or_mc_changed =
        minecraft_version_change.is_some() || loader_version_change.is_some();

    let has_structural_changes = !added_items.is_empty()
        || !removed_items.is_empty()
        || !config_changed_files.is_empty()
        || loader_or_mc_changed;

    let requires_github_release = has_structural_changes;

    let summary_message = if requires_github_release {
        let mut extras = Vec::new();
        if let Some((old_mc, new_mc)) = &minecraft_version_change {
            extras.push(format!("Minecraft: {} → {}", old_mc, new_mc));
        }
        if let Some((old_ld, new_ld)) = &loader_version_change {
            extras.push(format!("Loader: {} → {}", old_ld, new_ld));
        }
        let extra_str = if extras.is_empty() {
            String::new()
        } else {
            format!(" [{}]", extras.join(" | "))
        };
        format!(
            "Mudanças estruturais detectadas (+{} adicionados, -{} removidos, {} configs alteradas){}. Recomendado publicar nova versão no GitHub.",
            added_items.len(),
            removed_items.len(),
            config_changed_files.len(),
            extra_str
        )
    } else if !version_changed_items.is_empty() {
        format!(
            "Apenas {} mod(s)/pack(s) tiveram versão atualizada (nenhum item adicionado/removido e configs intactas). NÃO é necessário criar nova Release no GitHub — seus amigos podem atualizar usando 'Verificar Atualizações de Mods/Packs'!",
            version_changed_items.len()
        )
    } else {
        "Nenhuma alteração detectada em relação ao último manifesto publicado.".to_string()
    };

    ManifestDiff {
        has_structural_changes,
        added_items,
        removed_items,
        version_changed_items,
        config_changed_files,
        minecraft_version_change,
        loader_version_change,
        requires_github_release,
        summary_message,
    }
}

/// Empacota a pasta `config/` (e `options.txt` / `servers.dat` se habilitados) em `configs.zip`.
pub fn build_configs_zip(
    minecraft_dir: &Path,
    output_zip_path: &Path,
    include_options_txt: bool,
    include_servers_dat: bool,
) -> Result<()> {
    let file = File::create(output_zip_path)
        .with_context(|| format!("Erro ao criar {}", output_zip_path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    let config_dir = minecraft_dir.join("config");
    if config_dir.exists() {
        for entry in WalkDir::new(&config_dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Ok(rel) = path.strip_prefix(minecraft_dir) {
                    let rel_str = rel.to_string_lossy().replace('\\', "/");
                    zip.start_file(rel_str, options)?;
                    let mut f = File::open(path)?;
                    let mut buf = Vec::new();
                    f.read_to_end(&mut buf)?;
                    zip.write_all(&buf)?;
                }
            }
        }
    }

    if include_options_txt {
        let p = minecraft_dir.join("options.txt");
        if p.is_file() {
            zip.start_file("options.txt", options)?;
            let bytes = fs::read(&p)?;
            zip.write_all(&bytes)?;
        }
    }

    if include_servers_dat {
        let p = minecraft_dir.join("servers.dat");
        if p.is_file() {
            zip.start_file("servers.dat", options)?;
            let bytes = fs::read(&p)?;
            zip.write_all(&bytes)?;
        }
    }

    zip.finish()?;
    Ok(())
}

/// Empacota quaisquer itens marcados como `custom_override` (não encontrados no Modrinth/CurseForge)
/// em `custom_overrides.zip` mantendo sua estrutura (`mods/...`, `resourcepacks/...`, `shaderpacks/...`).
pub fn build_overrides_zip(
    minecraft_dir: &Path,
    items: &[PackItem],
    output_zip_path: &Path,
) -> Result<bool> {
    let overrides: Vec<&PackItem> = items
        .iter()
        .filter(|i| i.platform == ModPlatform::CustomOverride)
        .collect();

    if overrides.is_empty() {
        return Ok(false);
    }

    let file = File::create(output_zip_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for item in overrides {
        let rel_path = format!("{}/{}", item.category.folder_name(), item.file_name);
        let full_path = minecraft_dir.join(&rel_path);
        if full_path.is_file() {
            zip.start_file(&rel_path, options)?;
            let bytes = fs::read(&full_path)?;
            zip.write_all(&bytes)?;
        }
    }

    zip.finish()?;
    Ok(true)
}

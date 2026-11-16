use crate::api::build_http_client;
use crate::models::ModpackManifest;
use crate::scanner::{build_configs_zip, build_overrides_zip};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct GithubReleaseResponse {
    id: u64,
    tag_name: String,
    html_url: String,
    upload_url: String,
    assets: Vec<GithubReleaseAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubReleaseAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishReleaseResult {
    pub version_tag: String,
    pub release_url: String,
    pub manifest_path: String,
    pub configs_zip_path: String,
    pub uploaded_to_github: bool,
    pub message: String,
}

/// Caso o usuário não tenha colado manualmente o token na UI mas possua o GitHub CLI (`gh`)
/// autenticado em `~/.config/gh/hosts.yml`, obtém o `oauth_token` automaticamente.
pub fn resolve_effective_github_token(provided_token: &str) -> String {
    let trimmed = provided_token.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    if let Some(cfg_dir) = dirs::config_dir() {
        let hosts_path = cfg_dir.join("gh").join("hosts.yml");
        if let Ok(content) = fs::read_to_string(hosts_path) {
            for line in content.lines() {
                let t = line.trim();
                if let Some(tok) = t.strip_prefix("oauth_token:") {
                    let clean = tok.trim().trim_matches('"').trim_matches('\'');
                    if !clean.is_empty() {
                        return clean.to_string();
                    }
                }
            }
        }
    }
    String::new()
}

/// Busca o último `modpack-manifest.json` publicado nas Releases do GitHub,
/// ou lê o manifesto local salvo caso o repositório ainda não tenha releases públicas.
pub async fn fetch_latest_published_manifest(
    github_repo: &str,
    github_token: &str,
    local_cache_dir: &Path,
) -> Result<Option<ModpackManifest>> {
    let repo = github_repo.trim();
    let effective_token = resolve_effective_github_token(github_token);

    if !repo.is_empty() && repo.contains('/') && !repo.starts_with("meu-usuario/") {
        let client = build_http_client()?;
        let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);
        let mut req = client
            .get(&api_url)
            .header("Accept", "application/vnd.github+json");

        if !effective_token.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", effective_token));
        }

        if let Ok(resp) = req.send().await {
            if resp.status().is_success() {
                if let Ok(release) = resp.json::<GithubReleaseResponse>().await {
                    if let Some(asset) = release
                        .assets
                        .iter()
                        .find(|a| a.name == "modpack-manifest.json")
                    {
                        if let Ok(manifest_resp) =
                            client.get(&asset.browser_download_url).send().await
                        {
                            if manifest_resp.status().is_success() {
                                if let Ok(manifest) =
                                    manifest_resp.json::<ModpackManifest>().await
                                {
                                    let _ = fs::create_dir_all(local_cache_dir);
                                    let cache_file =
                                        local_cache_dir.join("last_published_manifest.json");
                                    if let Ok(serialized) =
                                        serde_json::to_string_pretty(&manifest)
                                    {
                                        let _ = fs::write(cache_file, serialized);
                                    }
                                    return Ok(Some(manifest));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Fallback: verifica se existe um manifesto publicado salvo localmente
    let cache_file = local_cache_dir.join("last_published_manifest.json");
    if cache_file.is_file() {
        let content = fs::read_to_string(&cache_file)?;
        if let Ok(manifest) = serde_json::from_str::<ModpackManifest>(&content) {
            return Ok(Some(manifest));
        }
    }

    Ok(None)
}

/// Empacota `configs.zip`, `custom_overrides.zip` e `modpack-manifest.json`, salva no cache local
/// e publica uma nova Release no GitHub anexando os pacotes.
pub async fn publish_new_release(
    mut manifest: ModpackManifest,
    source_minecraft_dir: &Path,
    staging_dir: &Path,
    github_repo: &str,
    github_token: &str,
    release_notes: &str,
) -> Result<PublishReleaseResult> {
    fs::create_dir_all(staging_dir)?;

    let raw_tag = manifest.version.trim().to_string();
    let version_tag = if raw_tag.starts_with('v') {
        raw_tag
    } else {
        format!("v{}", raw_tag)
    };
    manifest.version = version_tag.trim_start_matches('v').to_string();

    let configs_zip_path = staging_dir.join("configs.zip");
    build_configs_zip(
        source_minecraft_dir,
        &configs_zip_path,
        manifest.include_options_txt,
        manifest.include_servers_dat,
    )?;

    let overrides_zip_path = staging_dir.join("custom_overrides.zip");
    let has_overrides = build_overrides_zip(
        source_minecraft_dir,
        &manifest.items,
        &overrides_zip_path,
    )?;

    let repo = github_repo.trim();
    if !repo.is_empty() && repo.contains('/') {
        manifest.configs_download_url = Some(format!(
            "https://github.com/{}/releases/download/{}/configs.zip",
            repo, version_tag
        ));
        if has_overrides {
            manifest.overrides_download_url = Some(format!(
                "https://github.com/{}/releases/download/{}/custom_overrides.zip",
                repo, version_tag
            ));
        }
    }

    let manifest_path = staging_dir.join("modpack-manifest.json");
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    fs::write(&manifest_path, &manifest_json)?;

    // Atualiza referência local do último manifesto publicado
    let last_pub_path = staging_dir.join("last_published_manifest.json");
    fs::write(&last_pub_path, &manifest_json)?;

    let effective_token = resolve_effective_github_token(github_token);
    if !effective_token.is_empty()
        && !repo.is_empty()
        && repo.contains('/')
        && !repo.starts_with("meu-usuario/")
    {
        let client = build_http_client()?;
        let create_url = format!("https://api.github.com/repos/{}/releases", repo);

        let body = json!({
            "tag_name": version_tag,
            "name": format!("{} - {}", manifest.name, version_tag),
            "body": if release_notes.trim().is_empty() {
                format!(
                    "Atualização automática do Modpack **{}** ({})\n- Minecraft: `{}`\n- Loader: `{} {}`\n- Total de itens: `{}`",
                    manifest.name,
                    version_tag,
                    manifest.minecraft_version,
                    manifest.loader.name,
                    manifest.loader.version,
                    manifest.items.len()
                )
            } else {
                release_notes.to_string()
            },
            "draft": false,
            "prerelease": false
        });

        let resp = client
            .post(&create_url)
            .header("Authorization", format!("Bearer {}", effective_token))
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .await
            .context("Falha ao conectar à API do GitHub para criar Release")?;

        if !resp.status().is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("Erro da API do GitHub ao criar Release: {}", err_text);
        }

        let release: GithubReleaseResponse = resp.json().await?;
        let upload_base = release
            .upload_url
            .split('{')
            .next()
            .unwrap_or(&release.upload_url)
            .to_string();

        // Upload modpack-manifest.json
        upload_release_asset(
            &client,
            &upload_base,
            &effective_token,
            "modpack-manifest.json",
            "application/json",
            fs::read(&manifest_path)?,
        )
        .await?;

        // Upload configs.zip
        upload_release_asset(
            &client,
            &upload_base,
            &effective_token,
            "configs.zip",
            "application/zip",
            fs::read(&configs_zip_path)?,
        )
        .await?;

        // Upload custom_overrides.zip se existir
        if has_overrides && overrides_zip_path.is_file() {
            upload_release_asset(
                &client,
                &upload_base,
                &effective_token,
                "custom_overrides.zip",
                "application/zip",
                fs::read(&overrides_zip_path)?,
            )
            .await?;
        }

        return Ok(PublishReleaseResult {
            version_tag,
            release_url: release.html_url,
            manifest_path: manifest_path.to_string_lossy().to_string(),
            configs_zip_path: configs_zip_path.to_string_lossy().to_string(),
            uploaded_to_github: true,
            message: "Nova versão publicada com sucesso no GitHub Releases! O GitHub Actions compilará os binários automaticamente.".to_string(),
        });
    }

    Ok(PublishReleaseResult {
        version_tag: version_tag.clone(),
        release_url: format!("https://github.com/{}/releases/tag/{}", repo, version_tag),
        manifest_path: manifest_path.to_string_lossy().to_string(),
        configs_zip_path: configs_zip_path.to_string_lossy().to_string(),
        uploaded_to_github: false,
        message: "Manifesto (modpack-manifest.json) e pacote (configs.zip) gerados e salvos localmente!".to_string(),
    })
}

async fn upload_release_asset(
    client: &reqwest::Client,
    upload_base: &str,
    token: &str,
    file_name: &str,
    content_type: &str,
    bytes: Vec<u8>,
) -> Result<()> {
    let url = format!("{}?name={}", upload_base, file_name);
    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Accept", "application/vnd.github+json")
        .header("Content-Type", content_type)
        .body(bytes)
        .send()
        .await?;

    if !resp.status().is_success() {
        let err = resp.text().await.unwrap_or_default();
        anyhow::bail!("Falha ao enviar asset {}: {}", file_name, err);
    }
    Ok(())
}

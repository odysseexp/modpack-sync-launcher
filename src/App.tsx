import React, { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { check } from "@tauri-apps/plugin-updater";
import {
  AlertTriangle,
  ArrowUpCircle,
  CheckCircle2,
  CloudUpload,
  Cpu,
  Download,
  FileCode2,
  FolderCog,
  FolderOpen,
  GitBranch,
  KeyRound,
  Layers,
  Lock,
  Monitor,
  PackageCheck,
  RefreshCw,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  Wrench,
} from "lucide-react";
import { LauncherSetupModal } from "./components/LauncherSetupModal";
import {
  AdminScanResponse,
  AppSettings,
  DownloadProgressEvent,
  EngineUpdateStatus,
  InPlaceUpdateCandidate,
  ItemCategory,
  LauncherPathCandidate,
  ModpackManifest,
  PublishReleaseResult,
  SystemInitPayload,
} from "./types";

const DEFAULT_SETTINGS: AppSettings = {
  selected_launcher: "prism",
  selected_format: "flatpak",
  custom_launcher_path: "",
  target_instance_name: "Servidor Amigos",
  linux_theme_style: "gtk",
  os_theme_override: null,
  admin_source_path:
    "/home/deck/.var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances/Vanilla/minecraft",
  github_repo: "odysseexp/modpack-sync-launcher",
  github_token: "",
  curseforge_api_key:
    "$2a$10$4N9GcONHeSE4m41B.GbXgOlJsaCsYcOGQeZb.xakUtqhYZ2Je2sjm",
  include_options_txt: true,
  include_servers_dat: true,
};

function detectBrowserOs(): "windows" | "linux" | "macos" {
  const p = (navigator.platform || navigator.userAgent || "").toLowerCase();
  if (p.includes("win")) return "windows";
  if (p.includes("mac")) return "macos";
  return "linux";
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  const mb = kb / 1024;
  return `${mb.toFixed(2)} MB`;
}

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<"client" | "admin" | "settings">(
    "client"
  );
  // O painel Admin começa OCULTO na versão do cliente!
  // Desbloqueio secreto para o criador: Ctrl+Shift+A ou 5 cliques no ícone do app
  const [isAdminUnlocked, setIsAdminUnlocked] = useState<boolean>(false);
  const [, setLogoClickCount] = useState<number>(0);

  const [detectedOs, setDetectedOs] = useState<"windows" | "linux" | "macos">(
    detectBrowserOs()
  );
  const [detectedArch, setDetectedArch] = useState<string>("x86_64");
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [candidates, setCandidates] = useState<LauncherPathCandidate[]>([]);

  // Sempre abrir modal de confirmação de Launcher e formato na inicialização
  const [isLauncherModalOpen, setIsLauncherModalOpen] = useState<boolean>(true);

  // Estado do Manifesto e Progresso
  const [manifest, setManifest] = useState<ModpackManifest | null>(null);
  const [isLoadingManifest, setIsLoadingManifest] = useState<boolean>(false);
  const [categoryFilter, setCategoryFilter] = useState<ItemCategory | "all">(
    "all"
  );
  const [searchQuery, setSearchQuery] = useState<string>("");

  // Sincronização e Download Paralelo
  const [isSyncing, setIsSyncing] = useState<boolean>(false);
  const [progress, setProgress] = useState<DownloadProgressEvent | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  // Atualizações In-Place (Modrinth, CurseForge + Versão do Jogo / Fabric Loader)
  const [isCheckingInPlace, setIsCheckingInPlace] = useState<boolean>(false);
  const [inPlaceCandidates, setInPlaceCandidates] = useState<
    InPlaceUpdateCandidate[] | null
  >(null);
  const [engineStatus, setEngineStatus] = useState<EngineUpdateStatus | null>(
    null
  );

  // Modo Criador (Admin)
  const [versionInput, setVersionInput] = useState<string>("1.0.0");
  const [releaseNotes] = useState<string>("");
  const [isScanningAdmin, setIsScanningAdmin] = useState<boolean>(false);
  const [adminScanResult, setAdminScanResult] =
    useState<AdminScanResponse | null>(null);
  const [isPublishing, setIsPublishing] = useState<boolean>(false);
  const [publishResult, setPublishResult] =
    useState<PublishReleaseResult | null>(null);

  // Banner de Auto-Update (tauri-plugin-updater / Nova Release)
  const [appUpdateAvailable, setAppUpdateAvailable] = useState<{
    version: string;
    body?: string;
  } | null>(null);

  // Atalho secreto Ctrl+Shift+A para alternar o Modo Criador (Admin)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "a") {
        e.preventDefault();
        setIsAdminUnlocked((prev) => {
          const next = !prev;
          if (next) {
            setActiveTab("admin");
          } else {
            setActiveTab("client");
          }
          return next;
        });
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const handleLogoSecretClick = () => {
    setLogoClickCount((prev) => {
      const next = prev + 1;
      if (next >= 5) {
        setIsAdminUnlocked((u) => {
          const unlocked = !u;
          setActiveTab(unlocked ? "admin" : "client");
          return unlocked;
        });
        return 0;
      }
      return next;
    });
  };

  // Calcula o atributo de tema adaptativo
  const activeThemeAttr = useMemo(() => {
    const targetOs =
      (settings.os_theme_override as "windows" | "linux" | "macos" | null) ||
      detectedOs;
    if (targetOs === "windows") return "windows";
    if (targetOs === "macos") return "macos";
    return settings.linux_theme_style === "kde" ? "linux-kde" : "linux-gtk";
  }, [detectedOs, settings.os_theme_override, settings.linux_theme_style]);

  useEffect(() => {
    document.documentElement.setAttribute("data-os-theme", activeThemeAttr);
  }, [activeThemeAttr]);

  // Inicialização
  useEffect(() => {
    let unlistenFn: (() => void) | undefined;

    async function initApp() {
      try {
        const initData = await invoke<SystemInitPayload>(
          "get_system_info_and_paths",
          { osOverride: null }
        );
        setDetectedOs(initData.detected_os);
        setDetectedArch(initData.detected_arch);
        setSettings(initData.settings);
        setCandidates(initData.launcher_candidates);

        setIsLoadingManifest(true);
        const loadedManifest = await invoke<ModpackManifest>(
          "fetch_remote_manifest",
          {
            githubRepo: initData.settings.github_repo,
            githubToken: initData.settings.github_token,
            fallbackSourcePath: initData.settings.admin_source_path,
            curseforgeApiKey: initData.settings.curseforge_api_key,
          }
        );
        setManifest(loadedManifest);
        setVersionInput(loadedManifest.version || "1.0.0");
      } catch (err: any) {
        setErrorMessage(typeof err === "string" ? err : err?.message || null);
      } finally {
        setIsLoadingManifest(false);
      }

      try {
        const update = await check();
        if (update?.available) {
          setAppUpdateAvailable({
            version: update.version,
            body: update.body,
          });
        }
      } catch {
        // Ignora se ainda não houver release com latest.json
      }

      try {
        unlistenFn = await listen<DownloadProgressEvent>(
          "download-progress",
          (event) => {
            setProgress(event.payload);
          }
        );
      } catch {
        // Fallback fora do runtime Tauri
      }
    }

    initApp();
    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, []);

  const handleUpdateSettings = async (partial: Partial<AppSettings>) => {
    const next = { ...settings, ...partial };
    setSettings(next);
    try {
      await invoke("save_app_settings", { settings: next });
      if (partial.os_theme_override !== undefined) {
        const refreshed = await invoke<SystemInitPayload>(
          "get_system_info_and_paths",
          { osOverride: next.os_theme_override }
        );
        setDetectedOs(refreshed.detected_os);
        setCandidates(refreshed.launcher_candidates);
      }
    } catch {
      // Ignora em modo preview web
    }
  };

  const resolvedLauncherPath = useMemo(() => {
    if (settings.selected_format === "custom") {
      return settings.custom_launcher_path;
    }
    const matching = candidates.filter(
      (c) => c.launcher_id === settings.selected_launcher
    );
    const exact = matching.find((c) => c.format_id === settings.selected_format);
    return exact?.resolved_path || matching[0]?.resolved_path || "";
  }, [
    candidates,
    settings.selected_launcher,
    settings.selected_format,
    settings.custom_launcher_path,
  ]);

  const handleBrowseFolder = async (target: "launcher" | "admin_source") => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title:
          target === "launcher"
            ? "Selecione o diretório base do Launcher"
            : "Selecione a pasta minecraft de origem do Modpack",
      });
      if (typeof selected === "string" && selected) {
        if (target === "launcher") {
          handleUpdateSettings({
            selected_format: "custom",
            custom_launcher_path: selected,
          });
        } else {
          handleUpdateSettings({ admin_source_path: selected });
        }
      }
    } catch (err: any) {
      setErrorMessage(
        `Erro ao abrir seletor de pasta: ${err?.message || String(err)}`
      );
    }
  };

  // Sincronizar e Instalar Modpack (Modo Cliente)
  const handleSyncAndInstall = async () => {
    if (!manifest) return;
    if (!resolvedLauncherPath) {
      setIsLauncherModalOpen(true);
      return;
    }

    setErrorMessage(null);
    setStatusMessage(null);
    setIsSyncing(true);

    try {
      const installedDir = await invoke<string>("sync_install_modpack", {
        launcherId: settings.selected_launcher,
        baseLauncherPath: resolvedLauncherPath,
        instanceName: settings.target_instance_name,
        manifest,
        localSourceFallback: settings.admin_source_path,
      });
      setStatusMessage(
        `Modpack sincronizado e configurado com sucesso em: ${installedDir}`
      );
    } catch (err: any) {
      setErrorMessage(typeof err === "string" ? err : err?.message);
    } finally {
      setIsSyncing(false);
    }
  };

  // Verificar Atualizações In-Place de Mods/Packs + Versão do Jogo e Fabric Loader
  const handleCheckInPlaceUpdates = async () => {
    if (!manifest) return;
    setErrorMessage(null);
    setStatusMessage(null);
    setIsCheckingInPlace(true);

    try {
      const [updates, eng] = await Promise.all([
        invoke<InPlaceUpdateCandidate[]>("check_mods_in_place_updates", {
          manifest,
          curseforgeApiKey: settings.curseforge_api_key,
        }),
        invoke<EngineUpdateStatus>("check_engine_updates_cmd", {
          manifest,
        }),
      ]);

      setInPlaceCandidates(updates);
      setEngineStatus(eng);

      if (
        updates.length === 0 &&
        !eng.loader_update_available &&
        !eng.minecraft_update_available
      ) {
        setStatusMessage(
          `Tudo atualizado! Minecraft ${manifest.minecraft_version}, ${manifest.loader.name} ${manifest.loader.version} e todos os ${manifest.items.length} mods/packs estão na versão mais recente.`
        );
      }
    } catch (err: any) {
      setErrorMessage(typeof err === "string" ? err : err?.message);
    } finally {
      setIsCheckingInPlace(false);
    }
  };

  // Aplicar atualização da versão do Fabric Loader (e/ou Minecraft) in-place
  const handleApplyEngineUpdate = async (updateMc: boolean) => {
    if (!manifest || !engineStatus) return;
    setErrorMessage(null);
    try {
      const updated = await invoke<ModpackManifest>(
        "apply_engine_version_update",
        {
          launcherId: settings.selected_launcher,
          baseLauncherPath: resolvedLauncherPath,
          instanceName: settings.target_instance_name,
          manifest,
          newMinecraftVersion: updateMc ? engineStatus.latest_minecraft : null,
          newLoaderVersion: engineStatus.loader_update_available
            ? engineStatus.latest_loader_version
            : null,
          adminSourcePath: isAdminUnlocked ? settings.admin_source_path : null,
        }
      );
      setManifest(updated);
      setEngineStatus({
        ...engineStatus,
        current_minecraft: updated.minecraft_version,
        minecraft_update_available: false,
        current_loader_version: updated.loader.version,
        loader_update_available: false,
      });
      setStatusMessage(
        `Versão atualizada com sucesso na instância: Minecraft ${updated.minecraft_version} (${updated.loader.name} ${updated.loader.version})!`
      );
    } catch (err: any) {
      setErrorMessage(typeof err === "string" ? err : err?.message);
    }
  };

  const handleApplyInPlaceUpdates = async () => {
    if (!manifest || !inPlaceCandidates || inPlaceCandidates.length === 0)
      return;
    setErrorMessage(null);
    setIsSyncing(true);

    try {
      const count = await invoke<number>("apply_mods_in_place_updates", {
        launcherId: settings.selected_launcher,
        baseLauncherPath: resolvedLauncherPath,
        instanceName: settings.target_instance_name,
        manifest,
        candidates: inPlaceCandidates,
      });
      setStatusMessage(
        `${count} mod(s)/pack(s) atualizados in-place com sucesso para Minecraft ${manifest.minecraft_version}!`
      );
      setInPlaceCandidates(null);
    } catch (err: any) {
      setErrorMessage(typeof err === "string" ? err : err?.message);
    } finally {
      setIsSyncing(false);
    }
  };

  // Modo Criador (Admin): Escanear Pasta + Comparar com última versão publicada
  const handleScanAdminFolder = async () => {
    setErrorMessage(null);
    setStatusMessage(null);
    setPublishResult(null);
    setIsScanningAdmin(true);

    try {
      const res = await invoke<AdminScanResponse>("scan_admin_source", {
        sourcePath: settings.admin_source_path,
        versionTag: versionInput,
        includeOptionsTxt: settings.include_options_txt,
        includeServersDat: settings.include_servers_dat,
        githubRepo: settings.github_repo,
        githubToken: settings.github_token,
        curseforgeApiKey: settings.curseforge_api_key,
      });
      setAdminScanResult(res);
      setManifest(res.manifest);
    } catch (err: any) {
      setErrorMessage(typeof err === "string" ? err : err?.message);
    } finally {
      setIsScanningAdmin(false);
    }
  };

  // Modo Criador (Admin): Criar Nova Versão e Publicar no GitHub
  const handlePublishRelease = async () => {
    const targetManifest = adminScanResult?.manifest || manifest;
    if (!targetManifest) return;

    setErrorMessage(null);
    setIsPublishing(true);

    try {
      const updatedManifest: ModpackManifest = {
        ...targetManifest,
        version: versionInput,
      };
      const result = await invoke<PublishReleaseResult>(
        "publish_github_release",
        {
          manifest: updatedManifest,
          sourcePath: settings.admin_source_path,
          githubRepo: settings.github_repo,
          githubToken: settings.github_token,
          releaseNotes,
        }
      );
      setPublishResult(result);
      setStatusMessage(result.message);
    } catch (err: any) {
      setErrorMessage(typeof err === "string" ? err : err?.message);
    } finally {
      setIsPublishing(false);
    }
  };

  const filteredItems = useMemo(() => {
    if (!manifest) return [];
    return manifest.items.filter((item) => {
      const matchesCat =
        categoryFilter === "all" || item.category === categoryFilter;
      const q = searchQuery.trim().toLowerCase();
      const matchesQuery =
        !q ||
        item.display_name.toLowerCase().includes(q) ||
        item.file_name.toLowerCase().includes(q);
      return matchesCat && matchesQuery;
    });
  }, [manifest, categoryFilter, searchQuery]);

  const stats = useMemo(() => {
    const items = manifest?.items || [];
    return {
      mods: items.filter((i) => i.category === "mods").length,
      rps: items.filter((i) => i.category === "resourcepacks").length,
      sps: items.filter((i) => i.category === "shaderpacks").length,
      modrinth: items.filter((i) => i.platform === "modrinth").length,
      curseforge: items.filter((i) => i.platform === "curseforge").length,
      overrides: items.filter((i) => i.platform === "custom_override").length,
    };
  }, [manifest]);

  return (
    <div className="app-shell">
      {/* Modal de Seleção de Launcher sempre exibido ao iniciar */}
      <LauncherSetupModal
        isOpen={isLauncherModalOpen}
        detectedOs={detectedOs}
        settings={settings}
        candidates={candidates}
        onUpdateSettings={handleUpdateSettings}
        onBrowseCustomFolder={() => handleBrowseFolder("launcher")}
        onConfirm={() => setIsLauncherModalOpen(false)}
      />

      {/* Barra Superior Adaptativa por SO */}
      <header className="os-topbar">
        <div className="os-topbar-left">
          {activeThemeAttr === "macos" && (
            <div className="macos-traffic-lights" title="Controles macOS Aqua">
              <span className="traffic-dot close" />
              <span className="traffic-dot minimize" />
              <span className="traffic-dot maximize" />
            </div>
          )}
          <div
            className="app-brand"
            onClick={handleLogoSecretClick}
            style={{ cursor: "pointer" }}
            title="Modpack Sync Launcher"
          >
            <img
              src="/pack_icon.png"
              alt="Modpack Icon"
              style={{
                width: 24,
                height: 24,
                borderRadius: 6,
                objectFit: "cover",
              }}
            />
            <span>Modpack Sync Launcher</span>
          </div>
          <span className="os-badge">
            <Monitor size={12} />
            {activeThemeAttr === "windows" && "Windows 11 Fluent"}
            {activeThemeAttr === "linux-gtk" && "Linux GTK4 / Libadwaita"}
            {activeThemeAttr === "linux-kde" && "Linux Qt / KDE Breeze"}
            {activeThemeAttr === "macos" && "macOS ARM64 Vibrancy"}
          </span>
        </div>

        {/* Alternador rápido de Tema / Launcher no topo */}
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {detectedOs === "linux" && (
            <button
              type="button"
              className="btn"
              style={{ padding: "5px 10px", fontSize: "0.76rem" }}
              onClick={() =>
                handleUpdateSettings({
                  linux_theme_style:
                    settings.linux_theme_style === "gtk" ? "kde" : "gtk",
                })
              }
            >
              Tema Linux:{" "}
              {settings.linux_theme_style === "gtk"
                ? "GTK4 / GNOME"
                : "Qt / KDE Breeze"}
            </button>
          )}
          <button
            type="button"
            className="btn"
            style={{ padding: "5px 11px", fontSize: "0.78rem" }}
            onClick={() => setIsLauncherModalOpen(true)}
          >
            <FolderCog size={14} />
            Launcher: {settings.selected_launcher.toUpperCase()} (
            {settings.selected_format})
          </button>
        </div>
      </header>

      <div className="app-body">
        {/* Menu Lateral (Painel Admin oculto na versão do Cliente!) */}
        <aside className="sidebar">
          <div className="nav-group">
            <div className="nav-section-label">Modpack</div>
            <button
              type="button"
              className={`nav-item ${activeTab === "client" ? "active" : ""}`}
              onClick={() => setActiveTab("client")}
            >
              <Download size={18} />
              <span>Sincronizar & Jogar</span>
            </button>

            <button
              type="button"
              className={`nav-item ${activeTab === "settings" ? "active" : ""}`}
              onClick={() => setActiveTab("settings")}
            >
              <Settings size={18} />
              <span>Configurações & Temas</span>
            </button>

            {isAdminUnlocked && (
              <>
                <div className="nav-section-label" style={{ marginTop: 10 }}>
                  Administração (Criador)
                </div>
                <button
                  type="button"
                  className={`nav-item ${activeTab === "admin" ? "active" : ""}`}
                  onClick={() => setActiveTab("admin")}
                >
                  <CloudUpload size={18} />
                  <span>Modo Criador (Admin)</span>
                </button>
                <button
                  type="button"
                  className="nav-item"
                  onClick={() => {
                    setIsAdminUnlocked(false);
                    setActiveTab("client");
                  }}
                >
                  <Lock size={16} />
                  <span>Ocultar Painel Admin</span>
                </button>
              </>
            )}
          </div>

          {/* Card de Status da Instância no Rodapé da Sidebar */}
          <div
            className="card"
            style={{ padding: "12px", fontSize: "0.77rem", boxShadow: "none" }}
          >
            <div
              style={{
                fontWeight: 700,
                color: "var(--accent-primary)",
                marginBottom: 4,
                display: "flex",
                alignItems: "center",
                gap: 6,
              }}
            >
              <Cpu size={14} />
              <span>Ambiente Detectado</span>
            </div>
            <div style={{ color: "var(--text-secondary)", lineHeight: 1.45 }}>
              <div>
                SO: <strong>{detectedOs.toUpperCase()}</strong> ({detectedArch})
              </div>
              {manifest && (
                <>
                  <div>
                    Minecraft: <strong>{manifest.minecraft_version}</strong>
                  </div>
                  <div>
                    Loader:{" "}
                    <strong>
                      {manifest.loader.name} {manifest.loader.version}
                    </strong>
                  </div>
                </>
              )}
            </div>
          </div>
        </aside>

        {/* Área Principal */}
        <main className="main-content">
          {/* Banner de Nova Versão Publicada (Auto-Updater) */}
          {appUpdateAvailable && (
            <div className="update-banner">
              <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                <Sparkles size={22} color="#4ade80" />
                <div>
                  <div style={{ fontWeight: 700, fontSize: "0.92rem" }}>
                    Nova versão disponível no GitHub (v
                    {appUpdateAvailable.version})!
                  </div>
                  <div
                    style={{
                      fontSize: "0.8rem",
                      color: "var(--text-secondary)",
                    }}
                  >
                    {appUpdateAvailable.body ||
                      "O criador publicou alterações na lista de mods/configs do servidor."}
                  </div>
                </div>
              </div>
              <button
                type="button"
                className="btn btn-success"
                onClick={handleSyncAndInstall}
              >
                <ArrowUpCircle size={16} />
                Atualizar em 1 Clique
              </button>
            </div>
          )}

          {/* Alertas de Sucesso / Erro */}
          {statusMessage && (
            <div
              className="card"
              style={{
                borderColor: "rgba(34, 197, 94, 0.5)",
                background: "rgba(34, 197, 94, 0.12)",
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "12px 16px",
              }}
            >
              <CheckCircle2 size={18} color="#4ade80" />
              <span style={{ fontSize: "0.86rem" }}>{statusMessage}</span>
            </div>
          )}

          {errorMessage && (
            <div
              className="card"
              style={{
                borderColor: "rgba(239, 68, 68, 0.5)",
                background: "rgba(239, 68, 68, 0.12)",
                display: "flex",
                alignItems: "center",
                gap: 10,
                padding: "12px 16px",
              }}
            >
              <AlertTriangle size={18} color="#f87171" />
              <span style={{ fontSize: "0.86rem" }}>{errorMessage}</span>
            </div>
          )}

          {/* Barra de Progresso Ativa durante Download/Sincronização */}
          {(isSyncing || (progress && progress.phase !== "done")) &&
            progress && (
              <div className="card">
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    marginBottom: 8,
                    fontSize: "0.85rem",
                    fontWeight: 600,
                  }}
                >
                  <span>
                    [{progress.category.toUpperCase()}] {progress.file_name}
                  </span>
                  <span>
                    {progress.completed_files} / {progress.total_files} arquivos
                  </span>
                </div>
                <div className="progress-track">
                  <div
                    className="progress-fill"
                    style={{
                      width: `${
                        progress.total_files > 0
                          ? Math.min(
                              100,
                              Math.round(
                                (progress.completed_files /
                                  progress.total_files) *
                                  100
                              )
                            )
                          : 50
                      }%`,
                    }}
                  />
                </div>
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    marginTop: 6,
                    fontSize: "0.75rem",
                    color: "var(--text-secondary)",
                  }}
                >
                  <span>
                    Arquivo atual: {formatBytes(progress.downloaded_bytes)} /{" "}
                    {formatBytes(progress.total_bytes)}
                  </span>
                  <span>Downloads paralelos ativos (Modrinth & CurseForge)</span>
                </div>
              </div>
            )}

          {/* =================================================================
              ABA 1: MODO CLIENTE / AMIGOS
             ================================================================= */}
          {activeTab === "client" && (
            <>
              <div className="card">
                <div className="card-header">
                  <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
                    <img
                      src="/pack_icon.png"
                      alt="Pack Icon"
                      style={{
                        width: 48,
                        height: 48,
                        borderRadius: 10,
                        objectFit: "cover",
                        border: "1px solid var(--border-subtle)",
                      }}
                    />
                    <div>
                      <div className="card-title">
                        <PackageCheck size={20} color="var(--accent-primary)" />
                        <span>
                          {manifest
                            ? `${manifest.name} (v${manifest.version})`
                            : "Carregando Manifesto do Modpack..."}
                        </span>
                      </div>
                      <p className="card-subtitle" style={{ marginTop: 4 }}>
                        Instala e sincroniza automaticamente os Mods, Resource
                        Packs, Shader Packs, versão do Fabric/Minecraft e a pasta{" "}
                        <code>config/</code>.
                      </p>
                    </div>
                  </div>

                  <div style={{ display: "flex", gap: 10 }}>
                    <button
                      type="button"
                      className="btn"
                      disabled={isCheckingInPlace || isSyncing || !manifest}
                      onClick={handleCheckInPlaceUpdates}
                      title="Consulta Modrinth, CurseForge e Fabric Meta API para verificar novas versões de Mods, Fabric Loader e Minecraft"
                    >
                      <RefreshCw size={16} />
                      {isCheckingInPlace
                        ? "Consultando APIs..."
                        : "Verificar Atualizações (Mods & Fabric)"}
                    </button>

                    <button
                      type="button"
                      className="btn btn-primary"
                      disabled={isSyncing || isLoadingManifest || !manifest}
                      onClick={handleSyncAndInstall}
                    >
                      <Download size={17} />
                      {isSyncing
                        ? "Sincronizando..."
                        : "Sincronizar e Instalar Modpack"}
                    </button>
                  </div>
                </div>

                {/* Resumo da Instância e Destino */}
                <div className="grid-3" style={{ marginTop: 14 }}>
                  <div
                    style={{
                      padding: "12px",
                      background: "var(--bg-input)",
                      borderRadius: "var(--radius-control)",
                      border: "1px solid var(--border-subtle)",
                    }}
                  >
                    <div
                      style={{
                        fontSize: "0.72rem",
                        color: "var(--text-muted)",
                        fontWeight: 700,
                      }}
                    >
                      VERSÃO DO MINECRAFT & LOADER
                    </div>
                    <div
                      style={{
                        fontSize: "0.95rem",
                        fontWeight: 700,
                        marginTop: 4,
                      }}
                    >
                      Minecraft {manifest?.minecraft_version || "..."}{" "}
                      <span style={{ color: "var(--accent-primary)" }}>
                        ({manifest?.loader.name.toUpperCase()}{" "}
                        {manifest?.loader.version})
                      </span>
                    </div>
                  </div>

                  <div
                    style={{
                      padding: "12px",
                      background: "var(--bg-input)",
                      borderRadius: "var(--radius-control)",
                      border: "1px solid var(--border-subtle)",
                    }}
                  >
                    <div
                      style={{
                        fontSize: "0.72rem",
                        color: "var(--text-muted)",
                        fontWeight: 700,
                      }}
                    >
                      CONTEÚDO DO MODPACK
                    </div>
                    <div
                      style={{
                        fontSize: "0.9rem",
                        fontWeight: 600,
                        marginTop: 4,
                      }}
                    >
                      {stats.mods} Mods • {stats.rps} Resource Packs •{" "}
                      {stats.sps} Shaders
                    </div>
                  </div>

                  <div
                    style={{
                      padding: "12px",
                      background: "var(--bg-input)",
                      borderRadius: "var(--radius-control)",
                      border: "1px solid var(--border-subtle)",
                    }}
                  >
                    <div
                      style={{
                        fontSize: "0.72rem",
                        color: "var(--text-muted)",
                        fontWeight: 700,
                      }}
                    >
                      RESOLUÇÃO VIA HASH (MODRINTH / CURSEFORGE)
                    </div>
                    <div
                      style={{
                        display: "flex",
                        gap: 6,
                        marginTop: 5,
                        flexWrap: "wrap",
                      }}
                    >
                      <span className="badge badge-modrinth">
                        {stats.modrinth} Modrinth
                      </span>
                      <span className="badge badge-curseforge">
                        {stats.curseforge} CurseForge
                      </span>
                      {stats.overrides > 0 && (
                        <span className="badge badge-override">
                          {stats.overrides} Custom Override
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              </div>

              {/* Card de Atualização de Versão do Jogo / Fabric Loader (se detectada na Fabric Meta API) */}
              {engineStatus &&
                (engineStatus.loader_update_available ||
                  engineStatus.minecraft_update_available) && (
                  <div
                    className="card"
                    style={{ borderColor: "var(--accent-primary)" }}
                  >
                    <div className="card-header" style={{ marginBottom: 0 }}>
                      <div>
                        <div className="card-title">
                          <Sparkles size={19} color="var(--accent-primary)" />
                          <span>
                            Atualização de Motor Detectada (Fabric Meta API)
                          </span>
                        </div>
                        <p className="card-subtitle" style={{ marginTop: 4 }}>
                          {engineStatus.loader_update_available && (
                            <span>
                              • Novo{" "}
                              <strong>
                                {engineStatus.current_loader_name.toUpperCase()}{" "}
                                Loader
                              </strong>{" "}
                              disponível:{" "}
                              <code>{engineStatus.current_loader_version}</code>{" "}
                              →{" "}
                              <code style={{ color: "#4ade80" }}>
                                {engineStatus.latest_loader_version}
                              </code>
                              {"  "}
                            </span>
                          )}
                          {engineStatus.minecraft_update_available && (
                            <span>
                              • Versão estável mais recente do Minecraft:{" "}
                              <code>{engineStatus.current_minecraft}</code> →{" "}
                              <code style={{ color: "#60a5fa" }}>
                                {engineStatus.latest_minecraft}
                              </code>
                            </span>
                          )}
                        </p>
                      </div>

                      <div style={{ display: "flex", gap: 8 }}>
                        {engineStatus.loader_update_available && (
                          <button
                            type="button"
                            className="btn btn-primary"
                            onClick={() => handleApplyEngineUpdate(false)}
                          >
                            <ArrowUpCircle size={16} />
                            Atualizar Fabric Loader (
                            {engineStatus.latest_loader_version})
                          </button>
                        )}
                      </div>
                    </div>
                  </div>
                )}

              {/* Card de Atualizações In-Place de Mods Disponíveis (se houver) */}
              {inPlaceCandidates && inPlaceCandidates.length > 0 && (
                <div
                  className="card"
                  style={{ borderColor: "var(--accent-primary)" }}
                >
                  <div className="card-header">
                    <div>
                      <div className="card-title">
                        <ArrowUpCircle
                          size={19}
                          color="var(--accent-primary)"
                        />
                        <span>
                          {inPlaceCandidates.length} Atualizações de Mods/Packs
                          Encontradas!
                        </span>
                      </div>
                      <p className="card-subtitle">
                        Novas versões compatíveis com Minecraft{" "}
                        {manifest?.minecraft_version} ({manifest?.loader.name})
                        foram lançadas no Modrinth/CurseForge:
                      </p>
                    </div>
                    <button
                      type="button"
                      className="btn btn-primary"
                      disabled={isSyncing}
                      onClick={handleApplyInPlaceUpdates}
                    >
                      <Download size={16} />
                      Atualizar Todos In-Place ({inPlaceCandidates.length})
                    </button>
                  </div>
                  <div className="items-list" style={{ maxHeight: 180 }}>
                    {inPlaceCandidates.map((cand) => (
                      <div key={cand.current_file_name} className="item-row">
                        <div>
                          <strong>{cand.display_name}</strong>
                          <div
                            style={{
                              fontSize: "0.75rem",
                              color: "var(--text-secondary)",
                            }}
                          >
                            <code>{cand.current_file_name}</code> →{" "}
                            <code style={{ color: "#4ade80" }}>
                              {cand.latest_file_name}
                            </code>
                          </div>
                        </div>
                        <span
                          className={`badge ${
                            cand.platform === "modrinth"
                              ? "badge-modrinth"
                              : "badge-curseforge"
                          }`}
                        >
                          {cand.platform.toUpperCase()}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Lista de Itens do Manifesto */}
              <div className="card">
                <div className="card-header">
                  <div style={{ display: "flex", gap: 6 }}>
                    {(
                      [
                        ["all", `Todos (${manifest?.items.length || 0})`],
                        ["mods", `Mods (${stats.mods})`],
                        ["resourcepacks", `Resource Packs (${stats.rps})`],
                        ["shaderpacks", `Shader Packs (${stats.sps})`],
                      ] as const
                    ).map(([cat, label]) => (
                      <button
                        key={cat}
                        type="button"
                        className={`btn ${
                          categoryFilter === cat ? "btn-primary" : ""
                        }`}
                        style={{ padding: "6px 12px", fontSize: "0.8rem" }}
                        onClick={() => setCategoryFilter(cat)}
                      >
                        {label}
                      </button>
                    ))}
                  </div>

                  <div style={{ position: "relative", width: 260 }}>
                    <Search
                      size={15}
                      style={{
                        position: "absolute",
                        left: 10,
                        top: "50%",
                        transform: "translateY(-50%)",
                        color: "var(--text-muted)",
                      }}
                    />
                    <input
                      type="text"
                      className="input-field"
                      style={{ paddingLeft: 32, fontSize: "0.82rem" }}
                      placeholder="Buscar mod, shader ou arquivo..."
                      value={searchQuery}
                      onChange={(e) => setSearchQuery(e.target.value)}
                    />
                  </div>
                </div>

                <div className="items-list">
                  {filteredItems.map((item) => (
                    <div
                      key={`${item.category}-${item.file_name}`}
                      className="item-row"
                    >
                      <div style={{ display: "flex", flexDirection: "column" }}>
                        <span style={{ fontWeight: 600 }}>
                          {item.display_name}
                        </span>
                        <span
                          style={{
                            fontSize: "0.74rem",
                            color: "var(--text-secondary)",
                            fontFamily: "monospace",
                          }}
                        >
                          {item.file_name} • {formatBytes(item.file_size)} •
                          SHA1: {item.sha1.slice(0, 8)} • Murmur2:{" "}
                          {item.murmur2}
                        </span>
                      </div>

                      <div
                        style={{ display: "flex", alignItems: "center", gap: 8 }}
                      >
                        <span
                          className={`badge ${
                            item.platform === "modrinth"
                              ? "badge-modrinth"
                              : item.platform === "curseforge"
                              ? "badge-curseforge"
                              : "badge-override"
                          }`}
                        >
                          {item.platform === "modrinth"
                            ? "Modrinth API"
                            : item.platform === "curseforge"
                            ? "CurseForge API"
                            : "Custom Override"}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </>
          )}

          {/* =================================================================
              ABA 2: MODO CRIADOR (ADMIN) — OCULTO NA VERSÃO DO CLIENTE
             ================================================================= */}
          {activeTab === "admin" && isAdminUnlocked && (
            <>
              <div className="card">
                <div className="card-header">
                  <div>
                    <div className="card-title">
                      <Wrench size={20} color="var(--accent-primary)" />
                      <span>
                        Modo Criador (Admin) — Scanner SHA-1 / Murmur2 & GitHub
                        Releases
                      </span>
                    </div>
                    <p className="card-subtitle" style={{ marginTop: 4 }}>
                      Painel exclusivo do criador (desbloqueado via{" "}
                      <code>Ctrl+Shift+A</code> ou 5 cliques no ícone). Lê sua
                      instância do PrismLauncher, detecta alterações de mods,
                      configs, versão do Minecraft e do Fabric Loader e publica no
                      repositório <code>{settings.github_repo}</code>.
                    </p>
                  </div>
                </div>

                <div
                  style={{ display: "flex", flexDirection: "column", gap: 12 }}
                >
                  <div>
                    <label
                      style={{
                        fontSize: "0.76rem",
                        fontWeight: 700,
                        color: "var(--text-secondary)",
                        display: "block",
                        marginBottom: 5,
                      }}
                    >
                      PASTA DE ORIGEM DO CRIADOR (INSTÂNCIA PRISMLAUNCHER)
                    </label>
                    <div style={{ display: "flex", gap: 8 }}>
                      <input
                        type="text"
                        className="input-field"
                        value={settings.admin_source_path}
                        onChange={(e) =>
                          handleUpdateSettings({
                            admin_source_path: e.target.value,
                          })
                        }
                      />
                      <button
                        type="button"
                        className="btn"
                        onClick={() => handleBrowseFolder("admin_source")}
                      >
                        <FolderOpen size={16} />
                        Escolher
                      </button>
                    </div>
                  </div>

                  <div className="grid-3">
                    <div>
                      <label
                        style={{
                          fontSize: "0.76rem",
                          fontWeight: 700,
                          color: "var(--text-secondary)",
                          display: "block",
                          marginBottom: 5,
                        }}
                      >
                        NOVA VERSÃO / TAG DA RELEASE
                      </label>
                      <input
                        type="text"
                        className="input-field"
                        placeholder="Ex: 1.0.0"
                        value={versionInput}
                        onChange={(e) => setVersionInput(e.target.value)}
                      />
                    </div>

                    <div>
                      <label
                        style={{
                          fontSize: "0.76rem",
                          fontWeight: 700,
                          color: "var(--text-secondary)",
                          display: "block",
                          marginBottom: 5,
                        }}
                      >
                        REPOSITÓRIO GITHUB (OWNER/REPO)
                      </label>
                      <input
                        type="text"
                        className="input-field"
                        placeholder="odysseexp/modpack-sync-launcher"
                        value={settings.github_repo}
                        onChange={(e) =>
                          handleUpdateSettings({ github_repo: e.target.value })
                        }
                      />
                    </div>

                    <div>
                      <label
                        style={{
                          fontSize: "0.76rem",
                          fontWeight: 700,
                          color: "var(--text-secondary)",
                          display: "block",
                          marginBottom: 5,
                        }}
                      >
                        GITHUB TOKEN (OPCIONAL SE GH CLI LOGADO)
                      </label>
                      <input
                        type="password"
                        className="input-field"
                        placeholder="Automático via gh auth (odysseexp)"
                        value={settings.github_token}
                        onChange={(e) =>
                          handleUpdateSettings({ github_token: e.target.value })
                        }
                      />
                    </div>
                  </div>

                  <div
                    style={{
                      display: "flex",
                      justifyContent: "space-between",
                      alignItems: "center",
                      marginTop: 6,
                    }}
                  >
                    <div
                      style={{ display: "flex", gap: 18, fontSize: "0.83rem" }}
                    >
                      <label
                        style={{ display: "flex", alignItems: "center", gap: 6 }}
                      >
                        <input
                          type="checkbox"
                          checked={settings.include_options_txt}
                          onChange={(e) =>
                            handleUpdateSettings({
                              include_options_txt: e.target.checked,
                            })
                          }
                        />
                        Incluir <code>options.txt</code> no pacote
                      </label>
                      <label
                        style={{ display: "flex", alignItems: "center", gap: 6 }}
                      >
                        <input
                          type="checkbox"
                          checked={settings.include_servers_dat}
                          onChange={(e) =>
                            handleUpdateSettings({
                              include_servers_dat: e.target.checked,
                            })
                          }
                        />
                        Incluir <code>servers.dat</code> (IP do Servidor)
                      </label>
                    </div>

                    <div style={{ display: "flex", gap: 10 }}>
                      <button
                        type="button"
                        className="btn"
                        disabled={isScanningAdmin}
                        onClick={handleScanAdminFolder}
                      >
                        <RefreshCw size={16} />
                        {isScanningAdmin
                          ? "Calculando Hashes & Consultando APIs..."
                          : "1. Escanear Pasta e Detectar Mudanças"}
                      </button>

                      <button
                        type="button"
                        className="btn btn-primary"
                        disabled={isPublishing || isScanningAdmin || !manifest}
                        onClick={handlePublishRelease}
                      >
                        <CloudUpload size={17} />
                        {isPublishing
                          ? "Empacotando e Publicando..."
                          : "2. Criar Nova Versão e Publicar no GitHub"}
                      </button>
                    </div>
                  </div>
                </div>
              </div>

              {/* Resultado da Detecção Inteligente de Mudanças */}
              {adminScanResult && (
                <div
                  className="card"
                  style={{
                    borderColor: adminScanResult.diff.requires_github_release
                      ? "var(--warning)"
                      : "var(--success)",
                  }}
                >
                  <div className="card-header">
                    <div>
                      <div className="card-title">
                        <GitBranch
                          size={19}
                          color={
                            adminScanResult.diff.requires_github_release
                              ? "#f59e0b"
                              : "#22c55e"
                          }
                        />
                        <span>
                          Diagnóstico Inteligente de Mudanças{" "}
                          {adminScanResult.previous_version
                            ? `(Comparado à v${adminScanResult.previous_version})`
                            : "(Primeira Versão)"}
                        </span>
                      </div>
                      <p
                        className="card-subtitle"
                        style={{ marginTop: 4, fontWeight: 600 }}
                      >
                        {adminScanResult.diff.summary_message}
                      </p>
                    </div>
                    <span
                      className="badge"
                      style={{
                        background: adminScanResult.diff.requires_github_release
                          ? "rgba(245, 158, 11, 0.2)"
                          : "rgba(34, 197, 94, 0.2)",
                        color: adminScanResult.diff.requires_github_release
                          ? "#fbbf24"
                          : "#4ade80",
                        padding: "5px 10px",
                      }}
                    >
                      {adminScanResult.diff.requires_github_release
                        ? "Requer Nova Release no GitHub"
                        : "Atualização In-Place Suficiente (Sem Nova Release)"}
                    </span>
                  </div>

                  <div className="grid-3" style={{ marginTop: 10 }}>
                    <div className="item-row">
                      <span>Itens Adicionados (+)</span>
                      <strong style={{ color: "#4ade80" }}>
                        +{adminScanResult.diff.added_items.length}
                      </strong>
                    </div>
                    <div className="item-row">
                      <span>Itens Removidos (-)</span>
                      <strong style={{ color: "#f87171" }}>
                        -{adminScanResult.diff.removed_items.length}
                      </strong>
                    </div>
                    <div className="item-row">
                      <span>Arquivos em config/ Alterados</span>
                      <strong style={{ color: "#60a5fa" }}>
                        {adminScanResult.diff.config_changed_files.length}
                      </strong>
                    </div>
                  </div>
                </div>
              )}

              {publishResult && (
                <div className="card">
                  <div className="card-title" style={{ marginBottom: 8 }}>
                    <FileCode2 size={18} color="var(--accent-primary)" />
                    <span>
                      Arquivos Gerados da Release ({publishResult.version_tag})
                    </span>
                  </div>
                  <div
                    style={{
                      fontSize: "0.82rem",
                      color: "var(--text-secondary)",
                      fontFamily: "monospace",
                      lineHeight: 1.6,
                    }}
                  >
                    <div>Manifesto: {publishResult.manifest_path}</div>
                    <div>Pacote Configs: {publishResult.configs_zip_path}</div>
                    <div>Release URL: {publishResult.release_url}</div>
                  </div>
                </div>
              )}
            </>
          )}

          {/* =================================================================
              ABA 3: CONFIGURAÇÕES, CHAVE API CURSEFORGE E SIMULADOR DE TEMAS SO
             ================================================================= */}
          {activeTab === "settings" && (
            <div className="card">
              <div className="card-header">
                <div>
                  <div className="card-title">
                    <Layers size={20} color="var(--accent-primary)" />
                    <span>
                      Design Adaptativo por Sistema Operacional & API Keys
                    </span>
                  </div>
                  <p className="card-subtitle" style={{ marginTop: 4 }}>
                    O aplicativo detecta automaticamente o SO ao iniciar, mas
                    você pode alternar abaixo para visualizar os temas nativos de
                    Windows 11, Linux GTK4/Libadwaita, Linux KDE Breeze e macOS
                    ARM64.
                  </p>
                </div>
              </div>

              <div className="grid-2" style={{ marginTop: 12 }}>
                <div
                  style={{
                    padding: 14,
                    background: "var(--bg-input)",
                    borderRadius: "var(--radius-control)",
                    border: "1px solid var(--border-subtle)",
                  }}
                >
                  <div
                    style={{
                      fontWeight: 700,
                      fontSize: "0.86rem",
                      marginBottom: 10,
                    }}
                  >
                    Tema do Sistema Operacional (Auto-Detectado:{" "}
                    {detectedOs.toUpperCase()})
                  </div>
                  <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
                    <button
                      type="button"
                      className={`btn ${
                        settings.os_theme_override === null ? "btn-primary" : ""
                      }`}
                      onClick={() =>
                        handleUpdateSettings({ os_theme_override: null })
                      }
                    >
                      Automático ({detectedOs})
                    </button>
                    <button
                      type="button"
                      className={`btn ${
                        settings.os_theme_override === "windows"
                          ? "btn-primary"
                          : ""
                      }`}
                      onClick={() =>
                        handleUpdateSettings({ os_theme_override: "windows" })
                      }
                    >
                      Windows 11 Fluent
                    </button>
                    <button
                      type="button"
                      className={`btn ${
                        settings.os_theme_override === "linux" &&
                        settings.linux_theme_style === "gtk"
                          ? "btn-primary"
                          : ""
                      }`}
                      onClick={() =>
                        handleUpdateSettings({
                          os_theme_override: "linux",
                          linux_theme_style: "gtk",
                        })
                      }
                    >
                      Linux GTK4 / Libadwaita
                    </button>
                    <button
                      type="button"
                      className={`btn ${
                        settings.os_theme_override === "linux" &&
                        settings.linux_theme_style === "kde"
                          ? "btn-primary"
                          : ""
                      }`}
                      onClick={() =>
                        handleUpdateSettings({
                          os_theme_override: "linux",
                          linux_theme_style: "kde",
                        })
                      }
                    >
                      Linux Qt / KDE Breeze
                    </button>
                    <button
                      type="button"
                      className={`btn ${
                        settings.os_theme_override === "macos"
                          ? "btn-primary"
                          : ""
                      }`}
                      onClick={() =>
                        handleUpdateSettings({ os_theme_override: "macos" })
                      }
                    >
                      macOS ARM64 Aqua
                    </button>
                  </div>
                </div>

                <div
                  style={{
                    padding: 14,
                    background: "var(--bg-input)",
                    borderRadius: "var(--radius-control)",
                    border: "1px solid var(--border-subtle)",
                  }}
                >
                  <div
                    style={{
                      fontWeight: 700,
                      fontSize: "0.86rem",
                      marginBottom: 8,
                      display: "flex",
                      alignItems: "center",
                      gap: 6,
                    }}
                  >
                    <KeyRound size={16} color="var(--accent-primary)" />
                    <span>
                      Chave de API Oficial do CurseForge (Pré-configurada)
                    </span>
                  </div>
                  <input
                    type="text"
                    className="input-field"
                    value={settings.curseforge_api_key}
                    onChange={(e) =>
                      handleUpdateSettings({
                        curseforge_api_key: e.target.value,
                      })
                    }
                  />
                  <div
                    style={{
                      fontSize: "0.74rem",
                      color: "var(--text-secondary)",
                      marginTop: 6,
                      display: "flex",
                      alignItems: "center",
                      gap: 5,
                    }}
                  >
                    <ShieldCheck size={14} color="#4ade80" />
                    <span>
                      Usada no header <code>x-api-key</code> para resolver
                      Murmur2 fingerprints no CurseForge v1.
                    </span>
                  </div>
                </div>
              </div>
            </div>
          )}
        </main>
      </div>
    </div>
  );
};

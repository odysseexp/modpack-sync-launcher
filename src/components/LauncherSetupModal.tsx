import React from "react";
import {
  CheckCircle2,
  FolderOpen,
  Gamepad2,
  HardDrive,
  Laptop,
  Layers,
  Sparkles,
} from "lucide-react";
import { AppSettings, LauncherPathCandidate } from "../types";

interface LauncherSetupModalProps {
  isOpen: boolean;
  detectedOs: "windows" | "linux" | "macos";
  settings: AppSettings;
  candidates: LauncherPathCandidate[];
  onUpdateSettings: (partial: Partial<AppSettings>) => void;
  onBrowseCustomFolder: () => void;
  onConfirm: () => void;
}

const LAUNCHERS = [
  {
    id: "prism" as const,
    name: "PrismLauncher",
    subtitle: "Recomendado • Instâncias isoladas com suporte automático a Fabric/Forge/NeoForge",
  },
  {
    id: "atlauncher" as const,
    name: "ATLauncher",
    subtitle: "Cria e sincroniza automaticamente a instância dentro de instances/",
  },
  {
    id: "official" as const,
    name: 'Minecraft Launcher Oficial ("Minecraft normal")',
    subtitle: "Configura automaticamente o perfil no launcher_profiles.json",
  },
];

export const LauncherSetupModal: React.FC<LauncherSetupModalProps> = ({
  isOpen,
  detectedOs,
  settings,
  candidates,
  onUpdateSettings,
  onBrowseCustomFolder,
  onConfirm,
}) => {
  if (!isOpen) return null;

  const filteredCandidates = candidates.filter(
    (c) => c.launcher_id === settings.selected_launcher
  );

  const activeCandidate = filteredCandidates.find(
    (c) => c.format_id === settings.selected_format
  );

  const resolvedPath =
    settings.selected_format === "custom"
      ? settings.custom_launcher_path || "Selecione uma pasta personalizada..."
      : activeCandidate?.resolved_path ||
        filteredCandidates[0]?.resolved_path ||
        "";

  const osLabel =
    detectedOs === "windows"
      ? "Windows 11"
      : detectedOs === "macos"
      ? "macOS (Apple Silicon / ARM64)"
      : "Linux (SteamOS / Flatpak / Nativo)";

  return (
    <div className="modal-backdrop">
      <div className="modal-box">
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 8, marginBottom: 6 }}>
              <Sparkles size={18} color="var(--accent-primary)" />
              <span
                style={{
                  fontSize: "0.75rem",
                  fontWeight: 700,
                  textTransform: "uppercase",
                  letterSpacing: "0.06em",
                  color: "var(--accent-primary)",
                }}
              >
                Configuração Inicial • {osLabel}
              </span>
            </div>
            <h2 style={{ fontSize: "1.25rem", fontWeight: 700 }}>
              Qual Launcher você utiliza para jogar Minecraft?
            </h2>
            <p className="card-subtitle" style={{ marginTop: 4 }}>
              Sua última escolha fica salva automaticamente. Confirme ou altere seu Launcher e formato antes de sincronizar o modpack.
            </p>
          </div>
        </div>

        {/* Passo 1: Seleção do Launcher */}
        <div>
          <div className="nav-section-label" style={{ paddingLeft: 0, marginBottom: 8 }}>
            1. Escolha o seu Launcher
          </div>
          <div className="grid-3">
            {LAUNCHERS.map((launcher) => {
              const isSelected = settings.selected_launcher === launcher.id;
              return (
                <button
                  key={launcher.id}
                  type="button"
                  className={`launcher-card ${isSelected ? "selected" : ""}`}
                  onClick={() => {
                    const defaultFormat =
                      detectedOs === "linux"
                        ? settings.selected_format === "default"
                          ? "flatpak"
                          : settings.selected_format
                        : settings.selected_format === "custom"
                        ? "custom"
                        : "default";
                    onUpdateSettings({
                      selected_launcher: launcher.id,
                      selected_format: defaultFormat,
                    });
                  }}
                >
                  <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
                    <Gamepad2
                      size={20}
                      color={isSelected ? "var(--accent-primary)" : "var(--text-secondary)"}
                    />
                    {isSelected && <CheckCircle2 size={18} color="var(--accent-primary)" />}
                  </div>
                  <div style={{ fontWeight: 700, fontSize: "0.92rem", color: "var(--text-primary)" }}>
                    {launcher.name}
                  </div>
                  <div style={{ fontSize: "0.76rem", color: "var(--text-secondary)", lineHeight: 1.35 }}>
                    {launcher.subtitle}
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        {/* Passo 2: Seleção do Formato / Diretório de acordo com o SO */}
        <div>
          <div className="nav-section-label" style={{ paddingLeft: 0, marginBottom: 8 }}>
            2. Formato de Instalação e Diretório ({osLabel})
          </div>

          {detectedOs === "linux" ? (
            <div className="grid-3">
              {filteredCandidates.map((cand) => {
                const isSelected = settings.selected_format === cand.format_id;
                return (
                  <button
                    key={cand.format_id}
                    type="button"
                    className={`launcher-card ${isSelected ? "selected" : ""}`}
                    onClick={() => onUpdateSettings({ selected_format: cand.format_id })}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
                      <Layers size={17} color="var(--accent-primary)" />
                      <span
                        className="badge"
                        style={{
                          background: cand.exists
                            ? "rgba(34, 197, 94, 0.18)"
                            : "rgba(255, 255, 255, 0.08)",
                          color: cand.exists ? "#4ade80" : "var(--text-muted)",
                        }}
                      >
                        {cand.exists ? "Detectado" : "Padrão"}
                      </span>
                    </div>
                    <div style={{ fontWeight: 700, fontSize: "0.86rem", color: "var(--text-primary)" }}>
                      {cand.format_id === "flatpak" ? "Flatpak" : "AppImage / Nativo"}
                    </div>
                    <div
                      style={{
                        fontSize: "0.72rem",
                        color: "var(--text-secondary)",
                        wordBreak: "break-all",
                        fontFamily: "monospace",
                      }}
                    >
                      {cand.resolved_path}
                    </div>
                  </button>
                );
              })}

              <button
                type="button"
                className={`launcher-card ${
                  settings.selected_format === "custom" ? "selected" : ""
                }`}
                onClick={() => onUpdateSettings({ selected_format: "custom" })}
              >
                <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
                  <FolderOpen size={17} color="var(--accent-primary)" />
                </div>
                <div style={{ fontWeight: 700, fontSize: "0.86rem", color: "var(--text-primary)" }}>
                  Diretório Customizado
                </div>
                <div style={{ fontSize: "0.74rem", color: "var(--text-secondary)" }}>
                  Escolher manualmente a pasta de instalação no sistema
                </div>
              </button>
            </div>
          ) : (
            <div className="grid-2">
              {filteredCandidates.map((cand) => {
                const isSelected = settings.selected_format === "default";
                return (
                  <button
                    key={cand.format_id}
                    type="button"
                    className={`launcher-card ${isSelected ? "selected" : ""}`}
                    onClick={() => onUpdateSettings({ selected_format: "default" })}
                  >
                    <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
                      <HardDrive size={17} color="var(--accent-primary)" />
                      <span
                        className="badge"
                        style={{
                          background: cand.exists
                            ? "rgba(34, 197, 94, 0.18)"
                            : "rgba(255, 255, 255, 0.08)",
                          color: cand.exists ? "#4ade80" : "var(--text-muted)",
                        }}
                      >
                        {cand.exists ? "Detectado" : "Caminho Padrão"}
                      </span>
                    </div>
                    <div style={{ fontWeight: 700, fontSize: "0.88rem", color: "var(--text-primary)" }}>
                      {cand.label}
                    </div>
                    <div
                      style={{
                        fontSize: "0.73rem",
                        color: "var(--text-secondary)",
                        wordBreak: "break-all",
                        fontFamily: "monospace",
                      }}
                    >
                      {cand.resolved_path}
                    </div>
                  </button>
                );
              })}

              <button
                type="button"
                className={`launcher-card ${
                  settings.selected_format === "custom" ? "selected" : ""
                }`}
                onClick={() => onUpdateSettings({ selected_format: "custom" })}
              >
                <div style={{ display: "flex", justifyContent: "space-between", width: "100%" }}>
                  <FolderOpen size={17} color="var(--accent-primary)" />
                </div>
                <div style={{ fontWeight: 700, fontSize: "0.88rem", color: "var(--text-primary)" }}>
                  Diretório Customizado
                </div>
                <div style={{ fontSize: "0.75rem", color: "var(--text-secondary)" }}>
                  Selecionar outra unidade ou pasta de instalação personalizada
                </div>
              </button>
            </div>
          )}

          {settings.selected_format === "custom" && (
            <div style={{ display: "flex", gap: 8, marginTop: 12 }}>
              <input
                type="text"
                className="input-field"
                placeholder="Digite ou selecione o caminho da pasta do Launcher..."
                value={settings.custom_launcher_path}
                onChange={(e) =>
                  onUpdateSettings({ custom_launcher_path: e.target.value })
                }
              />
              <button
                type="button"
                className="btn"
                onClick={onBrowseCustomFolder}
                style={{ whiteSpace: "nowrap" }}
              >
                <FolderOpen size={16} />
                Procurar Pasta
              </button>
            </div>
          )}
        </div>

        {/* Resumo do caminho resolvido + Nome da Instância */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 14,
            padding: "12px 14px",
            background: "var(--bg-input)",
            borderRadius: "var(--radius-control)",
            border: "1px solid var(--border-subtle)",
          }}
        >
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: "0.73rem", color: "var(--text-muted)", fontWeight: 600 }}>
              DIRETÓRIO BASE SELECIONADO
            </div>
            <div
              style={{
                fontSize: "0.82rem",
                fontFamily: "monospace",
                color: "var(--text-primary)",
                marginTop: 2,
                wordBreak: "break-all",
              }}
            >
              {resolvedPath}
            </div>
          </div>

          <div style={{ width: 220 }}>
            <div style={{ fontSize: "0.73rem", color: "var(--text-muted)", fontWeight: 600, marginBottom: 3 }}>
              NOME DA INSTÂNCIA / PERFIL
            </div>
            <input
              type="text"
              className="input-field"
              style={{ padding: "6px 10px", fontSize: "0.82rem" }}
              value={settings.target_instance_name}
              onChange={(e) =>
                onUpdateSettings({ target_instance_name: e.target.value })
              }
            />
          </div>
        </div>

        {/* Rodapé */}
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 6, fontSize: "0.78rem", color: "var(--text-secondary)" }}>
            <Laptop size={15} />
            <span>Você pode testar como a tela fica no Windows/Linux/macOS nas Configurações.</span>
          </div>
          <button type="button" className="btn btn-primary" onClick={onConfirm}>
            <CheckCircle2 size={17} />
            Confirmar e Continuar
          </button>
        </div>
      </div>
    </div>
  );
};

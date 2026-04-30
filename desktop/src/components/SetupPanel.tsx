import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SectionCard, StatusBadge, ActionButton, Spinner } from "./shared";

interface AiTool {
  name: string;
  id: string;
  installed: boolean;
  hook_active: boolean;
}

interface RecommendedTool {
  name: string;
  id: string;
  installed: boolean;
  brew: string;
  purpose: string;
}

interface SystemStatus {
  accessibility: boolean;
  daemon_running: boolean;
  daemon_port: number;
  device_connected: boolean;
  transport: string;
}

interface SetupStatus {
  ai_tools: AiTool[];
  recommended: RecommendedTool[];
  system: SystemStatus;
}

function SetupPanel() {
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [actionInProgress, setActionInProgress] = useState<string | null>(null);

  const loadStatus = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<SetupStatus>("get_setup_status");
      setStatus(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadStatus();
  }, [loadStatus]);

  const installHook = useCallback(
    async (toolId: string) => {
      setActionInProgress(toolId);
      try {
        await invoke("setup_install", { toolId });
        await loadStatus();
      } catch (e) {
        alert(`Install failed: ${e}`);
      } finally {
        setActionInProgress(null);
      }
    },
    [loadStatus],
  );

  const uninstallHook = useCallback(
    async (toolId: string) => {
      setActionInProgress(toolId);
      try {
        await invoke("setup_uninstall", { toolId });
        await loadStatus();
      } catch (e) {
        alert(`Uninstall failed: ${e}`);
      } finally {
        setActionInProgress(null);
      }
    },
    [loadStatus],
  );

  const brewAction = useCallback(
    async (action: "install" | "uninstall", pkg: string) => {
      setActionInProgress(pkg);
      try {
        await invoke(action === "install" ? "brew_install" : "brew_uninstall", { package: pkg });
        await loadStatus();
      } catch (e) {
        alert(`${action} failed: ${e}`);
      } finally {
        setActionInProgress(null);
      }
    },
    [loadStatus],
  );

  if (loading) {
    return (
      <SectionCard style={{ width: "340px", textAlign: "center" }}>
        <Spinner /> Detecting tools...
      </SectionCard>
    );
  }

  if (error || !status) {
    return (
      <SectionCard style={{ width: "340px" }}>
        <div style={{ color: "var(--danger)", fontSize: "var(--font-size-md)", marginBottom: "8px" }}>
          Failed to load: {error}
        </div>
        <ActionButton variant="secondary" onClick={loadStatus}>
          Retry
        </ActionButton>
      </SectionCard>
    );
  }

  return (
    <div style={{ width: "340px" }}>
      {/* AI Tools */}
      <SectionCard title="AI Tools">
        {status.ai_tools.map((tool) => (
          <div
            key={tool.id}
            style={{
              padding: "8px 0",
              borderBottom: "1px solid #f3f4f6",
            }}
          >
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                marginBottom: "6px",
              }}
            >
              <span
                style={{
                  fontSize: "var(--font-size-md)",
                  fontWeight: 600,
                  color: "var(--text)",
                }}
              >
                {tool.name}
              </span>
              <StatusBadge
                variant={tool.installed ? "success" : "danger"}
                label={tool.installed ? "Installed" : "Not Found"}
              />
            </div>
            {tool.installed && (
              <div
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                }}
              >
                <span style={{ fontSize: "var(--font-size-xs)", color: "var(--text-muted)" }}>
                  Hook:{" "}
                  <StatusBadge
                    variant={tool.hook_active ? "success" : "muted"}
                    label={tool.hook_active ? "Active" : "Not Configured"}
                  />
                </span>
                <div style={{ display: "flex", gap: "6px" }}>
                  {tool.hook_active ? (
                    <>
                      <ActionButton
                        variant="secondary"
                        onClick={() => installHook(tool.id)}
                        disabled={actionInProgress === tool.id}
                        style={{ padding: "3px 8px", fontSize: "var(--font-size-xs)" }}
                      >
                        {actionInProgress === tool.id ? <Spinner size={12} /> : "Reinstall"}
                      </ActionButton>
                      <ActionButton
                        variant="danger"
                        onClick={() => uninstallHook(tool.id)}
                        disabled={actionInProgress === tool.id}
                        style={{ padding: "3px 8px", fontSize: "var(--font-size-xs)" }}
                      >
                        Uninstall
                      </ActionButton>
                    </>
                  ) : (
                    <ActionButton
                      variant="primary"
                      onClick={() => installHook(tool.id)}
                      disabled={actionInProgress === tool.id}
                      style={{ padding: "3px 8px", fontSize: "var(--font-size-xs)" }}
                    >
                      {actionInProgress === tool.id ? <Spinner size={12} /> : "Install Hook"}
                    </ActionButton>
                  )}
                </div>
              </div>
            )}
          </div>
        ))}
      </SectionCard>

      {/* Recommended Tools */}
      <SectionCard title="Recommended Tools">
        {status.recommended.map((tool) => (
          <div
            key={tool.id}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "space-between",
              padding: "8px 0",
              borderBottom: "1px solid #f3f4f6",
            }}
          >
            <div>
              <div
                style={{
                  fontSize: "var(--font-size-md)",
                  fontWeight: 600,
                  color: "var(--text)",
                }}
              >
                {tool.name}
              </div>
              <div
                style={{
                  fontSize: "var(--font-size-xs)",
                  color: "var(--text-muted)",
                }}
              >
                {tool.purpose}
              </div>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: "8px" }}>
              <StatusBadge
                variant={tool.installed ? "success" : "muted"}
                label={tool.installed ? "Installed" : "Not Found"}
              />
              {!tool.installed ? (
                <ActionButton
                  variant="ghost"
                  onClick={() => brewAction("install", tool.brew)}
                  disabled={actionInProgress === tool.brew}
                  style={{ padding: "3px 8px", fontSize: "var(--font-size-xs)" }}
                >
                  {actionInProgress === tool.brew ? <Spinner size={12} /> : "Install"}
                </ActionButton>
              ) : (
                <ActionButton
                  variant="danger"
                  onClick={() => brewAction("uninstall", tool.brew)}
                  disabled={actionInProgress === tool.brew}
                  style={{ padding: "3px 8px", fontSize: "var(--font-size-xs)" }}
                >
                  Uninstall
                </ActionButton>
              )}
            </div>
          </div>
        ))}
      </SectionCard>

      {/* System */}
      <SectionCard title="System">
        <div style={{ display: "flex", flexDirection: "column", gap: "8px" }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "var(--font-size-sm)", color: "var(--text-secondary)" }}>
              Accessibility
            </span>
            <StatusBadge
              variant={status.system.accessibility ? "success" : "danger"}
              label={status.system.accessibility ? "Granted" : "Not Granted"}
            />
          </div>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "var(--font-size-sm)", color: "var(--text-secondary)" }}>
              Daemon
            </span>
            <StatusBadge
              variant={status.system.daemon_running ? "success" : "danger"}
              label={
                status.system.daemon_running
                  ? `Running :${status.system.daemon_port}`
                  : "Not Running"
              }
            />
          </div>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: "var(--font-size-sm)", color: "var(--text-secondary)" }}>
              Device
            </span>
            <StatusBadge
              variant={status.system.device_connected ? "success" : "muted"}
              label={
                status.system.device_connected
                  ? `Connected (${status.system.transport})`
                  : "Disconnected"
              }
            />
          </div>
        </div>

        {!status.system.accessibility && (
          <ActionButton
            variant="secondary"
            fullWidth
            onClick={async () => {
              try {
                await invoke("open_accessibility_settings");
              } catch {
                alert("Open System Settings > Privacy & Security > Accessibility manually");
              }
            }}
            style={{ marginTop: "10px" }}
          >
            Open Accessibility Settings
          </ActionButton>
        )}
      </SectionCard>

      {/* About */}
      <SectionCard title="About">
        <div style={{ fontSize: "var(--font-size-sm)", color: "var(--text-secondary)" }}>
          Vibe Keyboard v0.1.0
        </div>
        <div
          style={{
            fontSize: "var(--font-size-xs)",
            color: "var(--text-muted)",
            fontFamily: "var(--font-mono)",
          }}
        >
          github.com/Adancurusul/vibe-keyboard
        </div>
      </SectionCard>
    </div>
  );
}

export default SetupPanel;

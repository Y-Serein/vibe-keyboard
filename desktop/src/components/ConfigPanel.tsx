import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SectionCard, Toggle, ActionButton, SettingRow, Spinner } from "./shared";
import type { DaemonConfig } from "../types";

const textareaStyle: React.CSSProperties = {
  width: "100%",
  minHeight: "60px",
  fontFamily: "var(--font-mono)",
  fontSize: "var(--font-size-sm)",
  padding: "8px",
  border: "1px solid var(--border)",
  borderRadius: "var(--radius-sm)",
  resize: "vertical",
  boxSizing: "border-box",
};

const inputStyle: React.CSSProperties = {
  width: "80px",
  fontFamily: "var(--font-mono)",
  fontSize: "var(--font-size-sm)",
  padding: "6px 8px",
  border: "1px solid var(--border)",
  borderRadius: "var(--radius-sm)",
};

function ConfigPanel() {
  const [config, setConfig] = useState<DaemonConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const debounceTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({});

  useEffect(() => {
    loadConfig();
    return () => {
      Object.values(debounceTimers.current).forEach(clearTimeout);
    };
  }, []);

  const loadConfig = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<DaemonConfig>("get_config");
      setConfig(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const saveConfig = useCallback((key: string, value: string) => {
    if (debounceTimers.current[key]) {
      clearTimeout(debounceTimers.current[key]);
    }
    debounceTimers.current[key] = setTimeout(async () => {
      try {
        await invoke("set_config", { key, value });
      } catch (e) {
        console.error("Failed to save config:", key, e);
      }
    }, 500);
  }, []);

  if (loading) {
    return (
      <SectionCard style={{ width: "340px", textAlign: "center" }}>
        <Spinner /> Loading config...
      </SectionCard>
    );
  }

  if (error) {
    return (
      <SectionCard style={{ width: "340px" }}>
        <div style={{ color: "var(--danger)", fontSize: "var(--font-size-md)", marginBottom: "8px" }}>
          Failed to load config: {error}
        </div>
        <ActionButton variant="secondary" onClick={loadConfig}>
          Retry
        </ActionButton>
      </SectionCard>
    );
  }

  const yolo = config?.yolo ?? {};
  const general = config?.general ?? {};

  return (
    <div style={{ width: "340px" }}>
      {/* YOLO Section */}
      <SectionCard title={"\u26A1 YOLO Mode"}>
        <SettingRow label="Active">
          <Toggle
            active={!!yolo.active}
            onChange={(val) => {
              setConfig((prev) => prev ? { ...prev, yolo: { ...prev.yolo, active: val } } : prev);
              saveConfig("yolo.active", String(val));
            }}
          />
        </SettingRow>

        <div style={{ marginBottom: "10px" }}>
          <label style={{ fontSize: "var(--font-size-sm)", fontWeight: 600, color: "var(--text-secondary)", display: "block", marginBottom: "4px" }}>
            Allow Rules (one per line)
          </label>
          <textarea
            style={textareaStyle}
            value={(yolo.allow ?? []).join("\n")}
            onChange={(e) => {
              const rules = e.target.value.split("\n");
              setConfig((prev) => prev ? { ...prev, yolo: { ...prev.yolo, allow: rules } } : prev);
              saveConfig("yolo.allow", rules.filter(r => r.trim()).join(","));
            }}
          />
        </div>

        <div>
          <label style={{ fontSize: "var(--font-size-sm)", fontWeight: 600, color: "var(--text-secondary)", display: "block", marginBottom: "4px" }}>
            Deny Rules (one per line)
          </label>
          <textarea
            style={textareaStyle}
            value={(yolo.deny ?? []).join("\n")}
            onChange={(e) => {
              const rules = e.target.value.split("\n");
              setConfig((prev) => prev ? { ...prev, yolo: { ...prev.yolo, deny: rules } } : prev);
              saveConfig("yolo.deny", rules.filter(r => r.trim()).join(","));
            }}
          />
        </div>

        <SettingRow label="Notify on Auto-Allow" description="Show notification when YOLO auto-allows a tool">
          <Toggle
            active={!!yolo.notify_auto_allow}
            onChange={(val) => {
              setConfig((prev) => prev ? { ...prev, yolo: { ...prev.yolo, notify_auto_allow: val } } : prev);
              saveConfig("yolo.notify_auto_allow", String(val));
            }}
          />
        </SettingRow>
      </SectionCard>

      {/* Network Section */}
      <SectionCard title="Network">
        <SettingRow label="Hook Port">
          <input
            style={inputStyle}
            type="number"
            value={general.hook_port ?? 19280}
            onChange={(e) => {
              const val = parseInt(e.target.value, 10);
              if (!isNaN(val)) {
                setConfig((prev) => prev ? { ...prev, general: { ...prev.general, hook_port: val } } : prev);
                saveConfig("general.hook_port", e.target.value);
              }
            }}
          />
        </SettingRow>
      </SectionCard>
    </div>
  );
}

export default ConfigPanel;

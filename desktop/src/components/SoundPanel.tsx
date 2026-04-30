import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SectionCard, Toggle, ActionButton, SettingRow, Spinner } from "./shared";
import type { DaemonConfig } from "../types";

interface SoundConfig {
  enabled: boolean;
  volume: number;
  muted: boolean;
  mapping: {
    permission_alert: string;
    session_complete: string;
    error: string;
    click: string;
  };
}

const EVENT_LABELS: { key: keyof SoundConfig["mapping"]; icon: string; label: string }[] = [
  { key: "permission_alert", icon: "\u26A0", label: "Permission Alert" },
  { key: "session_complete", icon: "\u2713", label: "Session Complete" },
  { key: "error", icon: "\u2718", label: "Error" },
  { key: "click", icon: "\u25CF", label: "Button Click" },
];

const SOUND_OPTIONS = [
  { value: "builtin:alert", label: "Alert (builtin)" },
  { value: "builtin:ding", label: "Ding (builtin)" },
  { value: "builtin:buzz", label: "Buzz (builtin)" },
  { value: "builtin:click", label: "Click (builtin)" },
  { value: "builtin:none", label: "None (silent)" },
];

function SoundPanel() {
  const [config, setConfig] = useState<SoundConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [customSounds, setCustomSounds] = useState<string[]>([]);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  const loadConfig = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<DaemonConfig>("get_config");
      const sound = data.sound;
      const mapping = sound?.mapping;
      setConfig({
        enabled: sound?.enabled ?? true,
        volume: sound?.volume ?? 80,
        muted: sound?.muted ?? false,
        mapping: {
          permission_alert: mapping?.permission_alert ?? "builtin:alert",
          session_complete: mapping?.session_complete ?? "builtin:ding",
          error: mapping?.error ?? "builtin:buzz",
          click: mapping?.click ?? "builtin:click",
        },
      });
      // Load custom sounds list
      try {
        const sounds = await invoke<{ builtin: string[]; custom: string[] }>("get_sounds");
        setCustomSounds(sounds.custom ?? []);
      } catch {
        // get_sounds may not exist yet
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadConfig();
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [loadConfig]);

  const saveConfig = useCallback(
    (key: string, value: string) => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(async () => {
        try {
          await invoke("set_config", { key, value });
        } catch (e) {
          console.error("Failed to save config:", key, e);
        }
      }, 300);
    },
    [],
  );

  const playSound = useCallback(async (soundType: string) => {
    try {
      await invoke("play_sound", { soundType });
    } catch (e) {
      console.warn("Play failed:", e);
    }
  }, []);

  const handleUpload = useCallback(async (file: File) => {
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      const result = await invoke<string>("upload_sound", {
        filename: file.name,
        data: Array.from(bytes),
      });
      if (result === "ok") {
        loadConfig();
      } else {
        alert("Upload failed: " + result);
      }
    } catch (e) {
      alert("Upload error: " + e);
    }
  }, [loadConfig]);

  if (loading) {
    return (
      <SectionCard style={{ width: "340px", textAlign: "center" }}>
        <Spinner /> Loading sound config...
      </SectionCard>
    );
  }

  if (error || !config) {
    return (
      <SectionCard style={{ width: "340px" }}>
        <div style={{ color: "var(--danger)", fontSize: "var(--font-size-md)", marginBottom: "8px" }}>
          Failed to load: {error}
        </div>
        <ActionButton variant="secondary" onClick={loadConfig}>
          Retry
        </ActionButton>
      </SectionCard>
    );
  }

  const allOptions = [
    ...SOUND_OPTIONS,
    ...customSounds.map((s) => ({ value: `custom:${s}`, label: `${s} (custom)` })),
  ];

  return (
    <div style={{ width: "340px" }}>
      {/* Master Volume */}
      <SectionCard title="Master Volume">
        <div style={{ display: "flex", alignItems: "center", gap: "12px", marginBottom: "12px" }}>
          <span style={{ fontSize: "20px" }}>{config.muted ? "\uD83D\uDD07" : "\uD83D\uDD0A"}</span>
          <input
            type="range"
            min={0}
            max={100}
            value={config.volume}
            disabled={config.muted}
            onChange={(e) => {
              const vol = parseInt(e.target.value, 10);
              setConfig((prev) => prev ? { ...prev, volume: vol } : prev);
              saveConfig("sound.volume", String(vol));
            }}
            style={{
              flex: 1,
              accentColor: "var(--primary)",
              opacity: config.muted ? 0.4 : 1,
            }}
          />
          <span
            style={{
              fontSize: "var(--font-size-sm)",
              fontFamily: "var(--font-mono)",
              color: "var(--text-secondary)",
              minWidth: "32px",
              textAlign: "right",
            }}
          >
            {config.volume}%
          </span>
        </div>
        <SettingRow label="Mute">
          <Toggle
            active={config.muted}
            onChange={(val) => {
              setConfig((prev) => prev ? { ...prev, muted: val } : prev);
              saveConfig("sound.muted", String(val));
            }}
          />
        </SettingRow>
      </SectionCard>

      {/* Event Sound Mapping */}
      <SectionCard title="Event Sound Mapping">
        {EVENT_LABELS.map(({ key, icon, label }) => (
          <div
            key={key}
            style={{
              display: "flex",
              alignItems: "center",
              gap: "8px",
              marginBottom: "10px",
              padding: "6px 0",
              borderBottom: "1px solid #f3f4f6",
            }}
          >
            <span style={{ fontSize: "14px", width: "20px", textAlign: "center" }}>{icon}</span>
            <span
              style={{
                flex: 1,
                fontSize: "var(--font-size-sm)",
                fontWeight: 600,
                color: "var(--text)",
              }}
            >
              {label}
            </span>
            <select
              value={config.mapping[key]}
              onChange={(e) => {
                const val = e.target.value;
                setConfig((prev) =>
                  prev
                    ? { ...prev, mapping: { ...prev.mapping, [key]: val } }
                    : prev,
                );
                saveConfig(`sound.mapping.${key}`, val);
              }}
              style={{
                width: "120px",
                fontSize: "var(--font-size-xs)",
                fontFamily: "var(--font-mono)",
                padding: "4px 6px",
                border: "1px solid var(--border)",
                borderRadius: "var(--radius-sm)",
              }}
            >
              {allOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
            <ActionButton
              variant="ghost"
              onClick={() => playSound(config.mapping[key])}
              style={{ padding: "4px 8px", fontSize: "var(--font-size-xs)" }}
            >
              {"\u25B6"}
            </ActionButton>
          </div>
        ))}
      </SectionCard>

      {/* Custom Sounds */}
      <SectionCard title="Custom Sounds">
        {customSounds.length > 0 && (
          <div style={{ marginBottom: "10px" }}>
            {customSounds.map((s) => (
              <div
                key={s}
                style={{
                  fontSize: "var(--font-size-sm)",
                  color: "var(--text-secondary)",
                  padding: "3px 0",
                  fontFamily: "var(--font-mono)",
                }}
              >
                {s}
              </div>
            ))}
          </div>
        )}
        <label>
          <ActionButton variant="secondary" fullWidth>
            Upload WAV File
          </ActionButton>
          <input
            type="file"
            accept=".wav"
            style={{ display: "none" }}
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) handleUpload(file);
            }}
          />
        </label>
        <p
          style={{
            fontSize: "var(--font-size-xs)",
            color: "var(--text-muted)",
            marginTop: "6px",
          }}
        >
          WAV files only. Uploaded to device for playback.
        </p>
      </SectionCard>
    </div>
  );
}

export default SoundPanel;

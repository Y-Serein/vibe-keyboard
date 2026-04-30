import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SectionCard, ActionButton } from "./shared";
import type { DaemonConfig } from "../types";

const KEY_MAP: Record<string, string> = {
  Enter: "enter", Escape: "escape", Tab: "tab",
  Backspace: "backspace", " ": "space", Delete: "backspace",
};

function keyEventToAction(e: KeyboardEvent): string {
  const parts: string[] = [];
  if (e.ctrlKey) parts.push("ctrl");
  if (e.altKey) parts.push("alt");
  if (e.metaKey) parts.push("cmd");
  if (e.shiftKey) parts.push("shift");
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if (parts.length > 0 && key.length === 1) return `${parts.join("_")}_${key}`;
  if (KEY_MAP[key]) return parts.length > 0 ? `${parts.join("_")}_${KEY_MAP[key]}` : KEY_MAP[key];
  return key.toLowerCase();
}

const ALL_BUTTONS = ["delete", "cancel", "mode", "session", "send", "voice"] as const;
type ButtonName = typeof ALL_BUTTONS[number];

const BUTTON_DEFAULTS: Record<ButtonName, string> = {
  delete: "ctrl_u", cancel: "", mode: "", session: "", send: "", voice: "",
};

const BUTTON_LABELS: Record<ButtonName, string> = {
  delete: "DELETE", cancel: "CANCEL", mode: "MODE",
  session: "SESSION", send: "SEND", voice: "VOICE",
};

const BUTTON_DESC: Record<ButtonName, string> = {
  delete: "Default: Ctrl+U (clear line)",
  cancel: "Default: Escape (stop AI) / Deny permission",
  mode: "Default: toggle YOLO/PLAN",
  session: "Default: jump to alert session",
  send: "Default: Enter / Allow permission",
  voice: "Configurable macro slot",
};

function BindingsPanel() {
  const [bindings, setBindings] = useState<Record<ButtonName, string>>({ ...BUTTON_DEFAULTS });
  const [recording, setRecording] = useState<ButtonName | null>(null);

  useEffect(() => {
    invoke<DaemonConfig>("get_config").then((cfg) => {
      const macros = cfg.macros;
      if (macros) {
        setBindings((prev) => {
          const next = { ...prev };
          for (const btn of ALL_BUTTONS) {
            if (macros[btn]) next[btn] = macros[btn];
          }
          return next;
        });
      }
    }).catch(() => {});
  }, []);

  const saveBinding = useCallback(async (key: ButtonName, value: string) => {
    setBindings((prev) => ({ ...prev, [key]: value }));
    try {
      await invoke("set_config", { key: `macros.${key}`, value });
    } catch (e) {
      console.warn("save failed:", e);
    }
  }, []);

  useEffect(() => {
    if (!recording) return;
    const handler = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape" && !e.ctrlKey && !e.metaKey) {
        setRecording(null);
        return;
      }
      const action = keyEventToAction(e);
      saveBinding(recording, action);
      setRecording(null);
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [recording, saveBinding]);

  const [editing, setEditing] = useState<ButtonName | null>(null);
  const [editText, setEditText] = useState("");

  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center" }}>
      <SectionCard title="Key Bindings" style={{ width: "340px" }}>

        {ALL_BUTTONS.map((btn) => (
          <div key={btn} style={{
            marginBottom: "var(--space-sm)", padding: "var(--space-sm) 10px",
            background: "var(--surface)", borderRadius: "var(--radius-sm)", border: "1px solid var(--border)",
          }}>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: "var(--font-size-md)", fontWeight: 700, color: "var(--text)" }}>
                  {BUTTON_LABELS[btn]}
                </div>
                <div style={{ fontSize: "var(--font-size-xs)", color: "var(--text-muted)" }}>
                  {BUTTON_DESC[btn]}
                </div>
                <div style={{ fontSize: "var(--font-size-sm)", color: "var(--primary)", fontFamily: "var(--font-mono)", marginTop: "2px" }}>
                  {bindings[btn] || "(default action)"}
                </div>
              </div>
              <div style={{ display: "flex", gap: "var(--space-xs)", marginLeft: "var(--space-sm)" }}>
                <ActionButton
                  variant={recording === btn ? "danger" : "primary"}
                  onClick={() => setRecording(recording === btn ? null : btn)}
                  style={{ padding: "4px 10px", fontSize: "var(--font-size-sm)" }}
                >
                  {recording === btn ? "Press..." : "Record"}
                </ActionButton>
                <ActionButton
                  variant="ghost"
                  onClick={() => { setEditing(editing === btn ? null : btn); setEditText(bindings[btn] || ""); }}
                  style={{ padding: "4px 8px", fontSize: "var(--font-size-sm)" }}
                >
                  Edit
                </ActionButton>
                {bindings[btn] && (
                  <ActionButton
                    variant="ghost"
                    onClick={() => saveBinding(btn, "")}
                    style={{ padding: "4px 8px", fontSize: "var(--font-size-sm)" }}
                  >
                    Clear
                  </ActionButton>
                )}
              </div>
            </div>
            {editing === btn && (
              <div style={{ marginTop: "6px", display: "flex", gap: "var(--space-xs)" }}>
                <input
                  autoFocus
                  value={editText}
                  onChange={(e) => setEditText(e.target.value)}
                  onKeyDown={(e) => {
                    e.stopPropagation(); // don't trigger Record
                    if (e.key === "Enter") {
                      saveBinding(btn, editText.trim());
                      setEditing(null);
                    } else if (e.key === "Escape") {
                      setEditing(null);
                    }
                  }}
                  placeholder="e.g. fn, cmd_space, ctrl_shift_a"
                  style={{
                    flex: 1, padding: "4px 8px", fontSize: "var(--font-size-sm)", borderRadius: "var(--radius-sm)",
                    border: "1px solid var(--primary)", fontFamily: "var(--font-mono)", outline: "none",
                  }}
                />
                <ActionButton
                  variant="primary"
                  onClick={() => { saveBinding(btn, editText.trim()); setEditing(null); }}
                  style={{ padding: "4px 8px", fontSize: "var(--font-size-sm)" }}
                >
                  Save
                </ActionButton>
              </div>
            )}
          </div>
        ))}

        <p style={{ fontSize: "var(--font-size-xs)", color: "var(--text-muted)", marginTop: "10px" }}>
          <b>Record</b>: press key combo (Escape to cancel, won't trigger system shortcuts in Tauri)<br/>
          <b>Edit</b>: type action name directly (fn, enter, cmd_space, ctrl_shift_a, etc.)
        </p>
      </SectionCard>
    </div>
  );
}

export default BindingsPanel;

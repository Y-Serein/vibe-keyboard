import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import Screen from "./components/Screen";
import VirtualKeyboard from "./components/VirtualKeyboard";
import ConfigPanel from "./components/ConfigPanel";
import BindingsPanel from "./components/BindingsPanel";
import SoundPanel from "./components/SoundPanel";
import SetupPanel from "./components/SetupPanel";
import ActivityLog from "./components/ActivityLog";
import { TabBar } from "./components/shared";

export interface SessionInfo {
  id: number;
  name: string;
  status: string;
  has_permission: boolean;
  source?: string;
  model?: string;
  cwd?: string;
  tokens_in?: number;
  tokens_out?: number;
  cost_usd?: number;
  context_pct?: number;
}

type ScreenMode = "standby" | "normal" | "select" | "allow" | "notify";
type Tab = "keyboard" | "config" | "bindings" | "sound" | "setup";

const TABS: { id: Tab; label: string }[] = [
  { id: "keyboard", label: "Keyboard" },
  { id: "config", label: "Config" },
  { id: "bindings", label: "Bindings" },
  { id: "sound", label: "Sound" },
  { id: "setup", label: "Setup" },
];

function App() {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [_activeIndex, setActiveIndex] = useState(0);
  const [connected, setConnected] = useState(false);
  const [screenMode, setScreenMode] = useState<ScreenMode>("standby");
  const [_allowSelection, setAllowSelection] = useState(0);
  const [modeYolo, setModeYolo] = useState(false);
  const [tab, setTab] = useState<Tab>("keyboard");

  useEffect(() => {
    const unlisten = listen<{ sessions: SessionInfo[]; active_index: number; yolo_active?: boolean }>(
      "session-update",
      (event) => {
        const s = event.payload.sessions;
        setSessions(s);
        setActiveIndex(event.payload.active_index);
        if (event.payload.yolo_active !== undefined) {
          setModeYolo(event.payload.yolo_active);
        }
        if (s.length === 0) {
          setScreenMode("standby");
        } else {
          const hasPermission = s.some((sess) => sess.has_permission);
          setScreenMode(hasPermission ? "allow" : "normal");
        }
      },
    );

    const unlistenConn = listen<boolean>("connection-status", (event) => {
      setConnected(event.payload);
      if (!event.payload) {
        setScreenMode("standby");
      }
    });

    return () => {
      unlisten.then((f) => f());
      unlistenConn.then((f) => f());
    };
  }, []);

  const handleButtonPress = useCallback(
    async (buttonId: string, action?: string) => {
      try {
        await invoke("button_press", { id: buttonId, action: action || null });
      } catch (e) {
        console.warn("button_press failed:", e);
      }
    },
    [],
  );

  const handleButtonDown = useCallback(
    (buttonId: string) => { handleButtonPress(buttonId, "down"); },
    [handleButtonPress],
  );
  const handleButtonUp = useCallback(
    (buttonId: string) => { handleButtonPress(buttonId, "up"); },
    [handleButtonPress],
  );

  const handleKnobAction = useCallback(
    async (action: string) => {
      if (action === "cw") {
        if (screenMode === "allow") {
          setAllowSelection((prev) => Math.min(prev + 1, 2));
        } else if (sessions.length > 0) {
          setActiveIndex((prev) => Math.min(prev + 1, sessions.length - 1));
        }
      } else if (action === "ccw") {
        if (screenMode === "allow") {
          setAllowSelection((prev) => Math.max(prev - 1, 0));
        } else if (sessions.length > 0) {
          setActiveIndex((prev) => Math.max(prev - 1, 0));
        }
      }
      try {
        await invoke("knob_action", { action });
      } catch (e) {
        console.warn("knob_action failed:", e);
      }
    },
    [screenMode, sessions.length],
  );

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        padding: "30px 20px",
        minHeight: "100vh",
        fontFamily: "var(--font-sans)",
      }}
    >
      <h1 style={{ color: "var(--text)", fontSize: "var(--font-size-xl)", marginBottom: "6px" }}>
        Vibe Keyboard
      </h1>
      <p style={{ color: "var(--text-secondary)", fontSize: "13px", marginBottom: "16px" }}>
        {connected ? "Connected to daemon" : "Disconnected"}{" "}
        <span style={{ color: connected ? "var(--success)" : "#f87171" }}>
          {connected ? "\u25CF" : "\u25CB"}
        </span>
        {connected && ` \u00B7 ${sessions.length} sessions`}
      </p>

      {!connected && (
        <div
          style={{
            background: "var(--danger)",
            color: "#fff",
            padding: "6px 16px",
            borderRadius: "var(--radius-sm)",
            fontSize: "var(--font-size-sm)",
            fontWeight: 600,
            marginBottom: "12px",
            width: "580px",
            textAlign: "center",
          }}
        >
          Disconnected from daemon — buttons disabled
        </div>
      )}

      <TabBar tabs={TABS} active={tab} onChange={setTab} />

      {tab === "keyboard" ? (
        <div
          style={{
            background: "var(--primary-light)",
            borderRadius: "var(--radius-xl)",
            padding: "16px",
            boxShadow: "var(--shadow-container)",
            width: "580px",
            maxWidth: "calc(100vw - 40px)",
          }}
        >
          <Screen
            mode={screenMode}
          />
          <VirtualKeyboard
            screenMode={screenMode}
            modeYolo={modeYolo}
            hasAlert={sessions.some((s) => s.has_permission)}
            notificationCount={sessions.filter((s) => s.has_permission || s.status === "Error" || s.status === "Done").length}
            onButtonPress={handleButtonPress}
            onButtonDown={handleButtonDown}
            onButtonUp={handleButtonUp}
            onKnobAction={handleKnobAction}
          />
          <ActivityLog />
        </div>
      ) : tab === "config" ? (
        <ConfigPanel />
      ) : tab === "bindings" ? (
        <BindingsPanel />
      ) : tab === "sound" ? (
        <SoundPanel />
      ) : tab === "setup" ? (
        <SetupPanel />
      ) : null}
    </div>
  );
}

export default App;

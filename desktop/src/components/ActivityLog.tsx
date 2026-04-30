import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";

function ActivityLog() {
  const [logs, setLogs] = useState<string[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        const data = await invoke<string[]>("get_activity_log");
        setLogs(data);
      } catch { /* ignore */ }
    }, 1000);
    return () => clearInterval(interval);
  }, []);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  return (
    <div
      ref={scrollRef}
      role="log"
      aria-live="polite"
      style={{
        background: "#1a1a2e",
        borderRadius: "8px",
        padding: "8px 10px",
        maxHeight: "100px",
        overflowY: "auto",
        fontFamily: "'SF Mono', 'Menlo', monospace",
        fontSize: "10px",
        lineHeight: "14px",
        color: "#8b949e",
        marginTop: "8px",
      }}
    >
      {logs.length === 0 ? (
        <div style={{ color: "#484f58" }}>No activity yet...</div>
      ) : (
        logs.map((line, i) => (
          <div key={i} style={{
            color: line.includes("failed") ? "#f85149" :
                   line.includes("down") ? "#58a6ff" :
                   line.includes("up") ? "#3fb950" :
                   "#8b949e",
          }}>
            {line}
          </div>
        ))
      )}
    </div>
  );
}

export default ActivityLog;

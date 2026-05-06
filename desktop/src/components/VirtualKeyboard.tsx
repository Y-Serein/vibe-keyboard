import { useRef, useEffect, useState } from "react";
import { PressableButton } from "./shared";

interface VirtualKeyboardProps {
  screenMode: "standby" | "normal" | "select" | "allow" | "notify";
  modeYolo: boolean;
  hasAlert: boolean;
  notificationCount?: number;
  onButtonPress: (buttonId: string, action?: string) => void;
  onButtonDown?: (buttonId: string) => void;
  onButtonUp?: (buttonId: string) => void;
  onKnobAction: (action: string) => void;
}

const sqBtnBase: React.CSSProperties = {
  width: "100%",
  aspectRatio: "3 / 2",
  borderRadius: "10px",
  border: "none",
  cursor: "pointer",
  fontFamily: "'Helvetica Neue', Arial, sans-serif",
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: "0.3px",
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  gap: "3px",
  position: "relative",
  transition: "transform 0.08s, box-shadow 0.08s",
  background: "linear-gradient(180deg, #f7f5f9 0%, #dedad4 100%)",
  boxShadow: "0 2px 6px rgba(0,0,0,0.18), inset 0 1px 0 rgba(255,255,255,0.9)",
  color: "#444",
  fontSize: "11px",
  padding: "6px",
};

const sqBtnRed: React.CSSProperties = {
  ...sqBtnBase,
  background: "linear-gradient(180deg, #ef4444 0%, #b91c1c 100%)",
  color: "#fff",
  boxShadow: "0 2px 6px rgba(185,28,28,0.4), inset 0 1px 0 rgba(255,255,255,0.3)",
};

const sqBtnDark: React.CSSProperties = {
  ...sqBtnBase,
  background: "linear-gradient(180deg, #4b5563 0%, #1f2937 100%)",
  color: "#fff",
  boxShadow: "0 2px 6px rgba(0,0,0,0.35), inset 0 1px 0 rgba(255,255,255,0.15)",
};

const longBtnBase: React.CSSProperties = {
  width: "100%",
  height: "60px",
  borderRadius: "12px",
  border: "none",
  cursor: "pointer",
  fontFamily: "'Helvetica Neue', Arial, sans-serif",
  fontWeight: 700,
  textTransform: "uppercase",
  letterSpacing: "0.5px",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  gap: "8px",
  position: "relative",
  transition: "transform 0.08s, box-shadow 0.08s",
  background: "linear-gradient(180deg, #f7f5f9 0%, #dedad4 100%)",
  boxShadow: "0 3px 8px rgba(0,0,0,0.18), inset 0 1px 0 rgba(255,255,255,0.9)",
  color: "#444",
  fontSize: "16px",
};

const pressedStyle: React.CSSProperties = {
  transform: "translateY(1px)",
  boxShadow: "0 1px 2px rgba(0,0,0,0.18)",
};

const ledStyle = (color: string, blink: boolean = false): React.CSSProperties => ({
  width: "7px",
  height: "7px",
  borderRadius: "50%",
  position: "absolute",
  top: "6px",
  right: "6px",
  background: color,
  boxShadow: `0 0 6px ${color}`,
  animation: blink ? "blink 1s infinite" : "none",
});

// ── Rotary Knob ────────────────────────────────────────────────────────────
// Drag to rotate; emits "cw"/"ccw" every ROTATE_STEP degrees.
// Click without drag = "press".

const ROTATE_STEP = 30;
const CLICK_MAX_DELTA = 5;
const CLICK_MAX_MS = 280;

function RotaryKnob({
  isAllow,
  onRotate,
  onPress,
}: {
  isAllow: boolean;
  onRotate: (action: "cw" | "ccw") => void;
  onPress: () => void;
}) {
  const knobRef = useRef<HTMLDivElement>(null);
  const onRotateRef = useRef(onRotate);
  const onPressRef = useRef(onPress);
  onRotateRef.current = onRotate;
  onPressRef.current = onPress;

  const drag = useRef({
    active: false,
    lastAngle: 0,
    accumulated: 0,
    totalAbs: 0,
    startTime: 0,
  });
  const [rotation, setRotation] = useState(0);
  const [grabbing, setGrabbing] = useState(false);

  const angleAt = (clientX: number, clientY: number) => {
    const el = knobRef.current;
    if (!el) return 0;
    const r = el.getBoundingClientRect();
    return (Math.atan2(clientY - (r.top + r.height / 2), clientX - (r.left + r.width / 2)) * 180) / Math.PI;
  };

  const onMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    drag.current = {
      active: true,
      lastAngle: angleAt(e.clientX, e.clientY),
      accumulated: 0,
      totalAbs: 0,
      startTime: Date.now(),
    };
    setGrabbing(true);
  };

  useEffect(() => {
    const onMove = (e: MouseEvent) => {
      const s = drag.current;
      if (!s.active) return;
      const a = angleAt(e.clientX, e.clientY);
      let d = a - s.lastAngle;
      if (d > 180) d -= 360;
      if (d < -180) d += 360;
      s.lastAngle = a;
      s.accumulated += d;
      s.totalAbs += Math.abs(d);
      setRotation((r) => r + d);
      while (s.accumulated > ROTATE_STEP) {
        onRotateRef.current("cw");
        s.accumulated -= ROTATE_STEP;
      }
      while (s.accumulated < -ROTATE_STEP) {
        onRotateRef.current("ccw");
        s.accumulated += ROTATE_STEP;
      }
    };
    const onUp = () => {
      const s = drag.current;
      if (!s.active) return;
      const wasClick = s.totalAbs < CLICK_MAX_DELTA && Date.now() - s.startTime < CLICK_MAX_MS;
      s.active = false;
      setGrabbing(false);
      if (wasClick) onPressRef.current();
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, []);

  const ringColor = isAllow ? "#34d399" : "#6b7280";

  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", width: "100%" }}>
      <div
        ref={knobRef}
        onMouseDown={onMouseDown}
        style={{
          width: "100%",
          maxWidth: 180,
          aspectRatio: "1 / 1",
          borderRadius: "50%",
          cursor: grabbing ? "grabbing" : "grab",
          userSelect: "none",
          background: "radial-gradient(circle at 35% 28%, #fafafa 0%, #d4d4d4 45%, #8a8a8a 95%)",
          boxShadow: `
            0 5px 14px rgba(0,0,0,0.28),
            inset 0 -3px 6px rgba(0,0,0,0.22),
            inset 0 3px 6px rgba(255,255,255,0.55)
          `,
          position: "relative",
          transform: `rotate(${rotation}deg)`,
          transition: drag.current.active ? "none" : "transform 0.18s ease-out",
          border: `2px solid ${ringColor}`,
        }}
      >
        {/* Knurled edge */}
        <div
          style={{
            position: "absolute",
            inset: 0,
            borderRadius: "50%",
            background:
              "repeating-conic-gradient(from 0deg, rgba(0,0,0,0.18) 0deg, rgba(0,0,0,0.18) 1.5deg, transparent 1.5deg, transparent 7deg)",
            WebkitMask: "radial-gradient(circle, transparent 0%, transparent 76%, black 82%, black 100%)",
            mask: "radial-gradient(circle, transparent 0%, transparent 76%, black 82%, black 100%)",
            pointerEvents: "none",
          }}
        />
        {/* Pointer notch */}
        <div
          style={{
            position: "absolute",
            top: "8%",
            left: "50%",
            transform: "translateX(-50%)",
            width: "5px",
            height: "16%",
            borderRadius: "2px",
            background: isAllow ? "#059669" : "#374151",
            pointerEvents: "none",
          }}
        />
        {/* Center cap */}
        <div
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            transform: "translate(-50%, -50%)",
            width: "22%",
            height: "22%",
            borderRadius: "50%",
            background: "radial-gradient(circle at 35% 35%, #e8e8e8 0%, #888 100%)",
            boxShadow: "inset 0 1px 2px rgba(0,0,0,0.35), 0 1px 1px rgba(255,255,255,0.4)",
            pointerEvents: "none",
          }}
        />
      </div>
      <div
        style={{
          fontSize: "9px",
          color: isAllow ? "#15803d" : "#888",
          fontWeight: 700,
          textTransform: "uppercase",
          letterSpacing: "0.5px",
          marginTop: "8px",
          textAlign: "center",
        }}
      >
        {isAllow ? "Allow Select" : "Drag · Click"}
      </div>
    </div>
  );
}

// ── Main Component ─────────────────────────────────────────────────────────

function VirtualKeyboard({
  screenMode,
  modeYolo,
  hasAlert,
  onButtonPress,
  onButtonDown,
  onButtonUp,
  onKnobAction,
  notificationCount = 0,
}: VirtualKeyboardProps) {
  const isAllow = screenMode === "allow";

  const longSendStyle: React.CSSProperties = isAllow
    ? {
        ...longBtnBase,
        background: "linear-gradient(180deg, #d1fae5 0%, #6ee7b7 100%)",
        color: "#065f46",
      }
    : longBtnBase;

  const click = (id: string) => onButtonPress(id);
  const modeStyle = modeYolo ? sqBtnDark : sqBtnBase;

  return (
    <div
      style={{
        display: "flex",
        gap: "16px",
        alignItems: "stretch",
      }}
    >
      {/* LEFT: 3×2 square button grid */}
      <div
        style={{
          flex: "1.15 1 0",
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gridTemplateRows: "1fr 1fr 1fr",
          gap: "10px",
        }}
      >
        {/* Row 1 — DELETE (red, hold-to-erase) | MODE */}
        <PressableButton
          style={sqBtnRed}
          pressedStyle={pressedStyle}
          onMouseDown={() => {
            if (!document.hasFocus()) return;
            onButtonDown?.("delete");
          }}
          onMouseUp={() => onButtonUp?.("delete")}
          aria-label="Delete - erase input"
        >
          <span style={{ fontSize: "17px" }}>{"⊗"}</span>
          DELETE
          <span style={{ fontSize: "8px", opacity: 0.85, fontWeight: 400 }}>ERASE</span>
        </PressableButton>

        <PressableButton
          style={modeStyle}
          pressedStyle={pressedStyle}
          onClick={() => click("mode")}
          aria-label="Mode - toggle Plan/YOLO"
        >
          <span style={ledStyle(modeYolo ? "#f59e0b" : "#22c55e", false)} />
          <span style={{ fontSize: "17px" }}>{"⚡"}</span>
          MODE
          <span style={{ fontSize: "8px", opacity: 0.7, fontWeight: 400 }}>
            {modeYolo ? "YOLO" : "PLAN"}
          </span>
        </PressableButton>

        {/* Row 2 — CANCEL (red) | NOTIFY */}
        <PressableButton
          style={sqBtnRed}
          pressedStyle={pressedStyle}
          onClick={() => click("cancel")}
          aria-label="Cancel - stop AI"
        >
          <span style={{ fontSize: "17px" }}>{"✘"}</span>
          CANCEL
          <span style={{ fontSize: "8px", opacity: 0.85, fontWeight: 400 }}>
            {isAllow ? "DENY" : "STOP"}
          </span>
        </PressableButton>

        <PressableButton
          style={sqBtnBase}
          pressedStyle={pressedStyle}
          onClick={() => click("session")}
          aria-label="Notify - notifications"
        >
          {(hasAlert || notificationCount > 0) && (
            <span style={ledStyle(hasAlert ? "#ef4444" : "#3b82f6", hasAlert)} />
          )}
          <span style={{ fontSize: "17px" }}>{"🔔"}</span>
          NOTIFY
          <span style={{ fontSize: "8px", color: "#888", fontWeight: 400 }}>
            {notificationCount > 0 ? `${notificationCount}` : ""}
          </span>
        </PressableButton>

        {/* Row 3 — VOICE | FN (placeholder) */}
        <PressableButton
          style={sqBtnBase}
          pressedStyle={pressedStyle}
          onClick={() => click("voice")}
          aria-label="Voice"
        >
          <span style={{ fontSize: "18px" }}>{"🎤"}</span>
          VOICE
        </PressableButton>

        <PressableButton
          style={{ ...sqBtnBase, color: "#aaa" }}
          pressedStyle={pressedStyle}
          onClick={() => click("fn")}
          aria-label="Function - reserved"
        >
          <span style={{ fontSize: "16px" }}>{"⋯"}</span>
          FN
          <span style={{ fontSize: "8px", color: "#bbb", fontWeight: 400 }}>MULTI</span>
        </PressableButton>
      </div>

      {/* RIGHT: knob (top) + long key (bottom) */}
      <div
        style={{
          flex: "1 1 0",
          display: "flex",
          flexDirection: "column",
          gap: "12px",
          justifyContent: "space-between",
        }}
      >
        <div
          style={{
            flex: "1 1 0",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            minHeight: 0,
          }}
        >
          <RotaryKnob
            isAllow={isAllow}
            onRotate={(a) => onKnobAction(a)}
            onPress={() => onKnobAction("press")}
          />
        </div>

        <PressableButton
          style={longSendStyle}
          pressedStyle={pressedStyle}
          onClick={() => click("send")}
          aria-label={isAllow ? "Send - allow" : "Send"}
        >
          <span style={{ fontSize: "20px" }}>
            {isAllow ? "✓" : "↵"}
          </span>
          {isAllow ? "ALLOW" : "SEND"}
        </PressableButton>
      </div>
    </div>
  );
}

export default VirtualKeyboard;
